use super::*;

pub const TEST_TOL: f64 = 1e-16;

#[test]
fn test_xafs_group_name_from_string() {
    let mut xafs_group = XASGroup::new();
    xafs_group.set_name("test".to_string());
    assert_eq!(xafs_group.name, Some("test".to_string()));
}

#[test]
fn test_xafs_group_name_from_str() {
    let mut xafs_group = XASGroup::new();
    xafs_group.set_name("test");
    assert_eq!(xafs_group.name, Some("test".to_string()));

    let name = String::from("test");

    let mut xafs_group = XASGroup::new();
    xafs_group.set_name(name.clone());
    assert_eq!(xafs_group.name, Some("test".to_string()));

    println!("name: {}", name);
}

#[test]
fn test_xafs_group_spectrum_from_vec() {
    let energy: Vec<f64> = vec![1.0, 2.0, 3.0];
    let mu: Array1<f64> = Array1::from_vec(vec![4.0, 5.0, 6.0]);
    let mut xafs_group = XASGroup::new();
    xafs_group.set_spectrum(energy, mu);
    assert_eq!(
        xafs_group.raw_energy,
        Some(Array1::from_vec(vec![1.0, 2.0, 3.0]))
    );
    assert_eq!(
        xafs_group.raw_mu,
        Some(Array1::from_vec(vec![4.0, 5.0, 6.0]))
    );
}
