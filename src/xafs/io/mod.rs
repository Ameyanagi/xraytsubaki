use crate::xafs::XASGroup;
use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};
use ndarray;
use std::error::Error;

pub fn load_spectrum(path: &String) -> Result<XASGroup, Box<dyn Error>> {
    let params = ReaderParams {
        comments: Some(b'#'),
        delimiter: Delimiter::WhiteSpace,
        ..Default::default()
    };

    let data = load_txt_f64(path, &params)?;
    let energy = data.get_col(0);
    let i0 = data.get_col(1);
    let it = data.get_col(2);
    let ir = data.get_col(3);
    let iff = data.get_col(4);

    let mut xafs_group = XASGroup::new();
    xafs_group.set_spectrum(
        energy,
        i0.iter()
            .zip(it)
            .map(|(i0, it)| (i0 / it).ln())
            .collect::<Vec<_>>(),
    );

    Ok(xafs_group)
}

mod tests {

    use super::*;

    const TOP_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

    #[test]
    fn test_load_spectrum() {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let result = load_spectrum(&path).unwrap();
        println!("{:?}", result);
    }
}
