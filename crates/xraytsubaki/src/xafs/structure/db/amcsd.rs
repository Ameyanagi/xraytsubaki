//! American Mineralogist Crystal Structure Database, in the SQLite layout
//! shipped by Larch/larixite (`amcsd_cif1.db` trimmed, `amcsd_cif2.db`
//! full): tables `cif`, `minerals`, `spacegroups` (H-M symbol + JSON list
//! of `x,y,z` operations), `cif_elements`, `publications`, `authors`.
//! Coordinates are stored as base64 of int32 × 4·10⁶.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine;
use rusqlite::{Connection, OpenFlags};

use super::{StructureHit, StructureQuery, StructureSource};
use crate::xafs::structure::cif::structure_from_cif;
use crate::xafs::structure::model::Structure;
use crate::xafs::structure::StructureError;

/// File name of the full database.
pub const AMCSD_FULL: &str = "amcsd_cif2.db";
/// File name of the trimmed database bundled with larixite.
pub const AMCSD_TRIM: &str = "amcsd_cif1.db";
/// Mirrors tried in order by [`download_amcsd`].
pub const SOURCE_URLS: [&str; 3] = [
    "https://docs.xrayabsorption.org/databases",
    "https://figshare.com/ndownloader/files/54545639",
    "https://millenia.cars.aps.anl.gov/xraylarch/downloads",
];

const FARRAY_SCALE: f64 = 4.0e6;

fn db_err<E: std::fmt::Display>(e: E) -> StructureError {
    StructureError::Database {
        reason: e.to_string(),
    }
}

/// Decode larixite's packed float arrays (`'0'` means absent).
pub fn decode_farray(text: &str) -> Vec<Option<f64>> {
    let text = text.trim();
    if text.is_empty() || text == "0" {
        return Vec::new();
    }
    let bytes = match base64::engine::general_purpose::STANDARD.decode(text) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    bytes
        .chunks_exact(4)
        .map(|c| {
            let v = i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64 / FARRAY_SCALE;
            if (v - 2.0).abs() < 1e-5 || (v - 3.0).abs() < 1e-5 {
                None
            } else {
                Some(v)
            }
        })
        .collect()
}

/// Read-only connection to an AMCSD database file.
pub struct Amcsd {
    conn: Connection,
    path: PathBuf,
}

impl std::fmt::Debug for Amcsd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Amcsd").field("path", &self.path).finish()
    }
}

/// A CIF record as stored in the database (cell + sites + symmetry).
#[derive(Debug, Clone)]
pub struct AmcsdRecord {
    pub id: i64,
    pub mineral: Option<String>,
    pub formula: String,
    pub hm_symbol: String,
    pub symmetry_xyz: Vec<String>,
    pub cell: [f64; 6],
    pub sites: Vec<String>,
    pub x: Vec<Option<f64>>,
    pub y: Vec<Option<f64>>,
    pub z: Vec<Option<f64>>,
    pub occupancy: Vec<Option<f64>>,
    pub url: Option<String>,
    pub publication: Option<String>,
}

impl AmcsdRecord {
    /// Re-create a CIF text like larixite's `CifStructure.ciftext`.
    pub fn to_cif(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("data_amcsd_{:07}\n", self.id));
        if let Some(m) = &self.mineral {
            out.push_str(&format!("_chemical_name_mineral '{m}'\n"));
        }
        out.push_str(&format!("_chemical_formula_sum '{}'\n", self.formula));
        if let Some(p) = &self.publication {
            out.push_str(&format!("_publ_section_title\n;\n{p}\n;\n"));
        }
        if let Some(u) = &self.url {
            out.push_str(&format!("_database_code_amcsd {}\n# {u}\n", self.id));
        }
        let c = self.cell;
        out.push_str(&format!(
            "_cell_length_a {}\n_cell_length_b {}\n_cell_length_c {}\n_cell_angle_alpha {}\n_cell_angle_beta {}\n_cell_angle_gamma {}\n",
            c[0], c[1], c[2], c[3], c[4], c[5]
        ));
        out.push_str(&format!(
            "_symmetry_space_group_name_H-M '{}'\nloop_\n_space_group_symop_operation_xyz\n",
            self.hm_symbol
        ));
        for op in &self.symmetry_xyz {
            out.push_str(&format!("'{op}'\n"));
        }
        let has_occ = !self.occupancy.is_empty();
        out.push_str(
            "loop_\n_atom_site_label\n_atom_site_fract_x\n_atom_site_fract_y\n_atom_site_fract_z\n",
        );
        if has_occ {
            out.push_str("_atom_site_occupancy\n");
        }
        for (i, label) in self.sites.iter().enumerate() {
            let f = |v: &Vec<Option<f64>>| match v.get(i).copied().flatten() {
                Some(x) => format!("{x:.6}"),
                None => "?".into(),
            };
            out.push_str(&format!(
                "{label} {} {} {}",
                f(&self.x),
                f(&self.y),
                f(&self.z)
            ));
            if has_occ {
                out.push_str(&format!(" {}", f(&self.occupancy)));
            }
            out.push('\n');
        }
        out
    }

    pub fn to_structure(&self) -> Result<Structure, StructureError> {
        let mut s = structure_from_cif(&self.to_cif())?;
        s.source = format!("amcsd:{}", self.id);
        if let Some(m) = &self.mineral {
            s.title = m.clone();
        }
        Ok(s)
    }
}

impl Amcsd {
    /// Open an existing database read-only.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StructureError> {
        let path = path.as_ref().to_path_buf();
        let conn =
            Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(db_err)?;
        let ok: i64 = conn
            .query_row(
                "select count(*) from sqlite_master where type='table' and name in ('cif','minerals','spacegroups')",
                [],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        if ok != 3 {
            return Err(StructureError::Database {
                reason: format!("{} is not an AMCSD database", path.display()),
            });
        }
        Ok(Self { conn, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn len(&self) -> Result<usize, StructureError> {
        self.conn
            .query_row("select count(*) from cif", [], |r| r.get::<_, i64>(0))
            .map(|n| n as usize)
            .map_err(db_err)
    }

    pub fn is_empty(&self) -> Result<bool, StructureError> {
        Ok(self.len()? == 0)
    }

    /// All mineral names.
    pub fn minerals(&self) -> Result<Vec<String>, StructureError> {
        let mut st = self
            .conn
            .prepare("select name from minerals order by name")
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    fn elements_of(&self, cif_id: i64) -> Result<Vec<String>, StructureError> {
        let mut st = self
            .conn
            .prepare("select element from cif_elements where cif_id = ?1")
            .map_err(db_err)?;
        let rows = st
            .query_map([cif_id.to_string()], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        let mut out: Vec<String> = rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?;
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// Fetch one record by AMCSD id.
    pub fn record(&self, id: i64) -> Result<AmcsdRecord, StructureError> {
        let row = self
            .conn
            .query_row(
                "select c.id, m.name, c.formula, s.hm_notation, s.symmetry_xyz, c.a, c.b, c.c, c.alpha, c.beta, c.gamma, c.atoms_sites, c.atoms_x, c.atoms_y, c.atoms_z, c.atoms_occupancy, c.amcsd_url, c.pub_title \
                 from cif c left join minerals m on m.id = c.mineral_id left join spacegroups s on s.id = c.spacegroup_id where c.id = ?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        [
                            r.get::<_, Option<String>>(5)?,
                            r.get::<_, Option<String>>(6)?,
                            r.get::<_, Option<String>>(7)?,
                            r.get::<_, Option<String>>(8)?,
                            r.get::<_, Option<String>>(9)?,
                            r.get::<_, Option<String>>(10)?,
                        ],
                        r.get::<_, Option<String>>(11)?,
                        r.get::<_, Option<String>>(12)?,
                        r.get::<_, Option<String>>(13)?,
                        r.get::<_, Option<String>>(14)?,
                        r.get::<_, Option<String>>(15)?,
                        r.get::<_, Option<String>>(16)?,
                        r.get::<_, Option<String>>(17)?,
                    ))
                },
            )
            .map_err(|e| StructureError::Database {
                reason: format!("AMCSD id {id}: {e}"),
            })?;
        let (id, mineral, formula, hm, symxyz, cell, sites, ax, ay, az, aocc, url, pub_title) = row;
        let cell_num = |v: &Option<String>| -> Result<f64, StructureError> {
            let text = v.clone().unwrap_or_default();
            let text = text
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .replace(',', ".");
            text.parse::<f64>().map_err(|_| StructureError::Database {
                reason: format!("AMCSD id {id}: bad cell value '{text}'"),
            })
        };
        let cell = [
            cell_num(&cell[0])?,
            cell_num(&cell[1])?,
            cell_num(&cell[2])?,
            cell_num(&cell[3])?,
            cell_num(&cell[4])?,
            cell_num(&cell[5])?,
        ];
        let sites: Vec<String> = sites
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default();
        let symmetry_xyz: Vec<String> = symxyz
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_else(|| vec!["x,y,z".into()]);
        let mineral = mineral.filter(|m| m != "<missing>" && !m.is_empty());
        Ok(AmcsdRecord {
            id,
            mineral,
            formula: formula.unwrap_or_default(),
            hm_symbol: hm.unwrap_or_else(|| "P 1".into()),
            symmetry_xyz,
            cell,
            sites,
            x: decode_farray(ax.as_deref().unwrap_or("0")),
            y: decode_farray(ay.as_deref().unwrap_or("0")),
            z: decode_farray(az.as_deref().unwrap_or("0")),
            occupancy: decode_farray(aocc.as_deref().unwrap_or("0")),
            url: url.filter(|u| !u.is_empty()),
            publication: pub_title.filter(|p| !p.is_empty() && p != "<missing>"),
        })
    }

    fn hit_for(
        &self,
        id: i64,
        mineral: Option<String>,
        formula: String,
        hm: Option<String>,
    ) -> Result<StructureHit, StructureError> {
        let mut extra = BTreeMap::new();
        extra.insert("amcsd_id".into(), id.to_string());
        Ok(StructureHit {
            id: id.to_string(),
            source: "amcsd".into(),
            formula: formula.split_whitespace().collect::<String>(),
            name: mineral.filter(|m| m != "<missing>" && !m.is_empty()),
            space_group: hm,
            elements: self.elements_of(id)?,
            extra,
        })
    }
}

impl StructureSource for Amcsd {
    fn name(&self) -> &str {
        "amcsd"
    }

    fn search(&self, query: &StructureQuery) -> Result<Vec<StructureHit>, StructureError> {
        let limit = if query.limit == 0 { 200 } else { query.limit };
        // Pre-filter in SQL by required elements and text, then apply the
        // full query on the hits.
        let mut sql = String::from(
            "select c.id, m.name, c.formula, s.hm_notation from cif c \
             left join minerals m on m.id = c.mineral_id \
             left join spacegroups s on s.id = c.spacegroup_id where 1=1",
        );
        let mut params: Vec<String> = Vec::new();
        for el in &query.elements {
            sql.push_str(" and c.id in (select cif_id from cif_elements where element = ?)");
            params.push(el.clone());
        }
        for el in &query.exclude {
            sql.push_str(" and c.id not in (select cif_id from cif_elements where element = ?)");
            params.push(el.clone());
        }
        if let Some(text) = query
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            let compact: String = text.split_whitespace().collect();
            sql.push_str(" and (lower(m.name) like ? or lower(replace(c.formula,' ','')) like ? or c.id = ?)");
            params.push(format!("%{}%", text.to_ascii_lowercase()));
            params.push(format!("%{}%", compact.to_ascii_lowercase()));
            params.push(text.to_string());
        }
        sql.push_str(" order by m.name, c.id limit ?");
        params.push((limit * 4).to_string());
        let mut st = self.conn.prepare(&sql).map_err(db_err)?;
        let rows = st
            .query_map(rusqlite::params_from_iter(params.iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(db_err)?;
        let mut hits = Vec::new();
        for row in rows {
            let (id, mineral, formula, hm) = row.map_err(db_err)?;
            let hit = self.hit_for(id, mineral, formula.unwrap_or_default(), hm)?;
            // Text already matched in SQL (name/formula/id); re-check element
            // constraints only.
            let mut q = query.clone();
            q.text = None;
            if q.matches(&hit) {
                hits.push(hit);
                if hits.len() >= limit {
                    break;
                }
            }
        }
        Ok(hits)
    }

    fn fetch(&self, hit: &StructureHit) -> Result<Structure, StructureError> {
        let id: i64 = hit.id.parse().map_err(|_| StructureError::Database {
            reason: format!("bad AMCSD id '{}'", hit.id),
        })?;
        self.record(id)?.to_structure()
    }
}

/// Download the full AMCSD database to `dest` (a file path), trying each
/// mirror. `progress(received_bytes, total_bytes)` is called as data
/// arrives. Requires the `materials-project` feature's HTTP client.
#[cfg(feature = "materials-project")]
pub fn download_amcsd<P: AsRef<Path>>(
    dest: P,
    progress: impl FnMut(u64, Option<u64>),
) -> Result<PathBuf, StructureError> {
    download_amcsd_cancellable(dest, progress, &std::sync::atomic::AtomicBool::new(false))
}

/// [`download_amcsd`] that stops early when `cancel` becomes `true`.
///
/// A cancelled download removes its partial file and returns
/// [`StructureError::Network`] with the reason `"cancelled"`. Requires the
/// `materials-project` feature's HTTP client.
#[cfg(feature = "materials-project")]
pub fn download_amcsd_cancellable<P: AsRef<Path>>(
    dest: P,
    mut progress: impl FnMut(u64, Option<u64>),
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<PathBuf, StructureError> {
    use std::sync::atomic::Ordering;
    let dest = dest.as_ref().to_path_buf();
    let mut last_err = None;
    for base in SOURCE_URLS {
        let url = if base.contains("figshare") {
            base.to_string()
        } else {
            format!("{base}/{AMCSD_FULL}")
        };
        match ureq::get(&url).call() {
            Ok(mut resp) => {
                let total = resp
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                let tmp = dest.with_extension("part");
                let mut file =
                    std::fs::File::create(&tmp).map_err(|source| StructureError::Io {
                        path: tmp.display().to_string(),
                        source,
                    })?;
                let mut reader = resp.body_mut().as_reader();
                let mut buf = vec![0u8; 1 << 16];
                let mut received = 0u64;
                loop {
                    let n = reader.read(&mut buf).map_err(|e| StructureError::Network {
                        reason: format!("{url}: {e}"),
                    })?;
                    if n == 0 {
                        break;
                    }
                    file.write_all(&buf[..n])
                        .map_err(|source| StructureError::Io {
                            path: tmp.display().to_string(),
                            source,
                        })?;
                    received += n as u64;
                    progress(received, total);
                    if cancel.load(Ordering::Relaxed) {
                        drop(file);
                        let _ = std::fs::remove_file(&tmp);
                        return Err(StructureError::Network {
                            reason: "cancelled".into(),
                        });
                    }
                }
                drop(file);
                std::fs::rename(&tmp, &dest).map_err(|source| StructureError::Io {
                    path: dest.display().to_string(),
                    source,
                })?;
                // Validate before declaring success.
                Amcsd::open(&dest)?;
                return Ok(dest);
            }
            Err(e) => last_err = Some(format!("{url}: {e}")),
        }
    }
    Err(StructureError::Network {
        reason: last_err.unwrap_or_else(|| "no mirrors".into()),
    })
}
