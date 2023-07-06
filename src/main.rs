use xraytsubaki::xafs;

fn main() {
    let mut group = xraytsubaki::xafs::XASGroup::new();
    let energy = ndarray::arr1(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    let mu = ndarray::arr1(&[1.0, 2.0, 3.0, 4.0, 5.0]);

    let new_energy_grid = ndarray::arr1(&[1.5, 2.5, 3.5, 4.5]);

    // group.set_spectrum(energy, mu);
    // let result = group.interpolate_spectrum(new_energy_grid);

    let energy: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let mu: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let new_energy_grid: Vec<f64> = vec![1.5, 2.5, 3.5, 4.5];

    let energy = xafs::interpolation(&energy, &mu, &new_energy_grid);

    println!("result: {:?}", energy);
}
