use std::array;

use ndarray::{array, Array};
use xraytsubaki::xafs::mathutils::MathUtils;

fn main() {
    let mut group = xraytsubaki::xafs::XASGroup::new();
    let energy = ndarray::arr1(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    let mu = ndarray::arr1(&[1.0, 2.0, 3.0, 4.0, 5.0]);

    let new_energy_grid = ndarray::arr1(&[1.5, 2.5, 3.5, 4.5]);

    // group.set_spectrum(energy, mu);
    // let result = group.interpolate_spectrum(new_energy_grid);

    let energy: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let mu: Vec<f64> = vec![2.0, 3.0, 3.0, 4.0, 5.0];
    let new_energy_grid: Vec<f64> = vec![1.5, 2.5, 3.5, 4.5];

    let energy = new_energy_grid.interpolate(&energy, &mu).unwrap();

    println!("result: {:?}", energy);

    group.raw_energy = Some(Array::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0]));
    group.raw_mu = Some(Array::from_vec(vec![2.0, 3.0, 3.0, 4.0, 5.0]));

    // group.interpolate_spectrum(new_energy_grid);

    // println!("result: {:?}", group.mu);

    // group.set_spectrum(
    //     vec![1.0, 2.0, 3.0, 4.0, 5.0, 2.0],
    //     vec![2.0, 3.0, 3.0, 4.0, 5.0, 2.0],
    // );

    // println!("result: {:?}", group.mu);
    // println!("result: {:?}", group.energy);
}
