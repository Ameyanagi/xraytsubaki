# Analysis of Failed Optimization

## My Assumption (WRONG)
I thought B-spline basis functions depend only on:
- Knot vector
- Order
- Evaluation points

NOT on coefficients.

## Why This Is WRONG
Looking at the original code (line 761-767):
```rust
let spline_jacobian = -splev_jacobian(
    self.knots.data.as_vec().clone(),
    self.coefs.data.as_vec().clone(),  // <-- USES ACTUAL COEFFICIENTS!
    self.order,
    self.kout.data.as_vec().clone(),
    3,
);
```

The original code passes `self.coefs` (the CURRENT coefficients being optimized).

## My "Optimization" (line 416-422)
```rust
let precomputed_basis = -splev_jacobian(
    knots.clone(),
    vec![0.0; num_coefs],  // <-- DUMMY COEFFICIENTS!
    order,
    kout.to_vec(),
    3,
);
```

I used ZERO coefficients, assuming the basis doesn't depend on them.

## The Real Issue
The `splev_jacobian` function apparently DOES depend on coefficients, meaning:
1. The Jacobian changes as coefficients change during optimization
2. Precomputing with dummy coefficients gives WRONG Jacobian values
3. Wrong Jacobian → wrong optimization direction → slow/wrong convergence

## Conclusion
My optimization was based on a FUNDAMENTAL MISUNDERSTANDING of how B-spline Jacobians work.
The coefficients DO matter, so precomputation is NOT possible in this straightforward way.

## Next Steps
1. Verify the original code's performance (running now)
2. Understand WHY the user said "after nalgebra was 4 sec" 
3. Maybe there's a DIFFERENT optimization that was already done?
