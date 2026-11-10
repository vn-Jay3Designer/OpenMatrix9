// SPDX-License-Identifier: LGPL-2.1-or-later
//! Exact surface edits by knot operations: split at an isocurve (Rhino 5 Split with the
//! Isocurve option) and move the seam of a closed surface (SrfSeam). The shapes are
//! unchanged; only the parameterisation is cut or turned.
use crate::exact::{Nc, clamp_to};
use crate::extend::{assemble, lines};
use om9_addon_abi::session::Geo;

fn normalised(curves: Vec<Nc>) -> Vec<Nc> {
    curves
        .into_iter()
        .map(|mut c| {
            let (a, b) = (c.knots[0], *c.knots.last().unwrap());
            c.knots.iter_mut().for_each(|v| *v = (*v - a) / (b - a));
            c
        })
        .collect()
}

/// Split `g` at the isocurve `t` (normalised) of direction `dir` (0: u = t): two surfaces.
pub fn split_at(g: &Geo, dir: usize, t: f64) -> Result<(Geo, Geo), String> {
    if !(1e-9..=1.0 - 1e-9).contains(&t) {
        return Err("the isocurve is on the surface's edge".into());
    }
    let (curves, degree, _, knots) = lines(g, dir).ok_or("not a NURBS surface")?;
    let curves = normalised(curves);
    let first: Option<Vec<Nc>> = curves.iter().map(|c| clamp_to(c, 0.0, t)).collect();
    let second: Option<Vec<Nc>> = curves.iter().map(|c| clamp_to(c, t, 1.0)).collect();
    let (first, second) = (first.ok_or("could not split")?, second.ok_or("could not split")?);
    Ok((assemble(&first, dir, degree, knots.clone()), assemble(&second, dir, degree, knots)))
}

/// Move the seam of a surface closed in direction `dir` to `t` (normalised).
pub fn move_seam(g: &Geo, dir: usize, t: f64) -> Result<Geo, String> {
    let (curves, degree, _, knots) = lines(g, dir).ok_or("not a NURBS surface")?;
    let curves = normalised(curves);
    if !curves.iter().all(|c| c.closed(1e-6)) {
        return Err("the surface is not closed in that direction".into());
    }
    let moved: Vec<Nc> = curves.iter().map(|c| c.with_seam(t)).collect();
    Ok(assemble(&moved, dir, degree, knots))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exact::{self, H};
    use crate::nurbs::Srf;
    use om9_addon_abi::session::dist;

    fn cylinder() -> Geo {
        let ring = exact::arc([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 3.0, std::f64::consts::TAU);
        let rows: Vec<Vec<H>> = ring.points.iter().map(|c| vec![*c, [c[0], c[1], 4.0 * c[3], c[3]]]).collect();
        exact::surface(&rows, [2, 1], [ring.knots.clone(), vec![0.0, 0.0, 1.0, 1.0]])
    }

    #[test]
    fn split_and_seam_keep_the_shape_exactly() {
        let g = cylinder();
        let s = Srf::of(&g).unwrap();
        let (a, b) = split_at(&g, 0, 0.3).unwrap();
        let (fa, fb) = (Srf::of(&a).unwrap(), Srf::of(&b).unwrap());
        // The pieces meet at the isocurve and are on the cylinder.
        for j in 0..=4 {
            let v = j as f64 / 4.0;
            assert!(dist(fa.at_st(1.0, v), s.at_st(0.3, v)) < 1e-9);
            assert!(dist(fb.at_st(0.0, v), s.at_st(0.3, v)) < 1e-9);
        }
        for f in [&fa, &fb] {
            for i in 0..=20 {
                let p = f.at_st(i as f64 / 20.0, 0.5);
                assert!((p[0].hypot(p[1]) - 3.0).abs() < 1e-9);
            }
        }
        // Split along v (height) too.
        let (lo, hi) = split_at(&g, 1, 0.25).unwrap();
        assert!((Srf::of(&lo).unwrap().at_st(0.4, 1.0)[2] - 1.0).abs() < 1e-9);
        assert!((Srf::of(&hi).unwrap().at_st(0.4, 0.0)[2] - 1.0).abs() < 1e-9);
        // Seam moved: the new start is the old point at 0.3; still the cylinder.
        let m = move_seam(&g, 0, 0.3).unwrap();
        let fm = Srf::of(&m).unwrap();
        assert!(dist(fm.at_st(0.0, 0.5), s.at_st(0.3, 0.5)) < 1e-9);
        for i in 0..=40 {
            let p = fm.at_st(i as f64 / 40.0, 0.5);
            assert!((p[0].hypot(p[1]) - 3.0).abs() < 1e-9);
        }
        // Not closed across v.
        assert!(move_seam(&g, 1, 0.5).is_err());
    }
}
