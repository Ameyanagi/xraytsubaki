#![allow(unused_imports)]
#![allow(unused_variables)]

use ndarray::{array, Array1};
use xraytsubaki::xafs::mathutils::MathUtils;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use xraytsubaki::xafs::io;
    use xraytsubaki::xafs::xafsutils::find_energy_step;

    const TOP_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let mut xafs_test_group = io::load_spectrum(&path).unwrap();

    xafs_test_group.normalize()?;
    xafs_test_group.calc_background()?;

    let k = xafs_test_group.get_k().unwrap();
    let chi = xafs_test_group.get_chi().unwrap();

    // println!("k: {:?}", k);
    // println!("chi: {:?}", chi);
    // println!("e0: {:?}", xafs_test_group.normalization.unwrap().get_e0());

    println!("{:?}", xafs_test_group);

    Ok(())

    // let mut group = xraytsubaki::xafs::XASGroup::new();
    // let energy = ndarray::arr1(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    // let mu = ndarray::arr1(&[1.0, 2.0, 3.0, 4.0, 5.0]);

    // let new_energy_grid = ndarray::arr1(&[1.5, 2.5, 3.5, 4.5]);

    // // group.set_spectrum(energy, mu);
    // // let result = group.interpolate_spectrum(new_energy_grid);

    // let energy: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    // let mu: Vec<f64> = vec![2.0, 3.0, 3.0, 4.0, 5.0];
    // let new_energy_grid: Vec<f64> = vec![1.5, 2.5, 3.5, 4.5];

    // let energy = new_energy_grid.interpolate(&energy, &mu).unwrap();

    // println!("result: {:?}", energy);

    // group.raw_energy = Some(Array::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0]));
    // group.raw_mu = Some(Array::from_vec(vec![2.0, 3.0, 3.0, 4.0, 5.0]));

    // group.interpolate_spectrum(new_energy_grid);

    // println!("result: {:?}", group.mu);

    // group.set_spectrum(
    //     vec![1.0, 2.0, 3.0, 4.0, 5.0, 2.0],
    //     vec![2.0, 3.0, 3.0, 4.0, 5.0, 2.0],
    // );

    // println!("result: {:?}", group.mu);
    // println!("result: {:?}", group.energy);
}
