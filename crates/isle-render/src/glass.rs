// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian
//! Port of isle @ a29a7c4 theme.js LOOKS.glass and glass.js CODE, with lens/spread disabled.
//! The same graph feeds Windows' compositor and the reference evaluator used by previews/tests.
pub const LO: f32 = 0.03;
pub const SPAN: f32 = 0.64;
pub const GAMMA: f32 = 1.;
pub const DEEP: f32 = 0.52;
pub const KNEE: f32 = 0.22;
pub const SHEEN: f32 = 0.03;
pub const GLOW: f32 = 0.05;
pub const REACH: f32 = 4.; // island.js GLASS_GLOW_REACH
pub const RIM: f32 = 0.5;
pub const RIM_WIDTH: f32 = 1.1;
pub const LUMA: [f32;3] = [0.2126,0.7152,0.0722];

// Port of glass.js's tone expression (isle @ a29a7c4), without the screen-copy or lens.
pub fn tone(rgb: [f32;3], expanded: bool) -> [f32;3] {
    let l=rgb.iter().zip(LUMA).map(|(c,w)|c*w).sum::<f32>();
    let t=((l-KNEE)/(1.-KNEE)).clamp(0.,1.); let smooth=t*t*(3.-2.*t);
    let sat=if expanded {1.15}else{0.95};
    rgb.map(|c|(LO+SPAN*l.max(0.).powf(GAMMA)*(1.-DEEP*smooth)+(c-l)*SPAN*sat).clamp(0.,1.))
}

#[derive(Clone)]
pub enum Effect { Blur(f32), Matrix([f32;20],bool), Arithmetic([f32;4],bool) }
pub struct Node {pub effect: Effect, pub inputs: Vec<usize>}
pub const BACKDROP: usize = usize::MAX;
fn matrix(scale: [f32;3], bias: f32) -> [f32;20] {
    let mut m=[0.;20]; for i in 0..3 {m[i*4+i]=scale[i];m[16+i]=bias;} m[19]=1.;m
}
// Port of glass.js CODE as a static Direct2D graph (gamma is 1 in theme.js):
// smoothstep = t*t*(3-2*t). No CPU backdrop access or continuously running animation.
pub fn tone_graph(expanded: bool) -> Vec<Node> {
    let mut nodes=Vec::new();
    let mut add=|effect,inputs:Vec<usize>| {let i=nodes.len();nodes.push(Node{effect,inputs});i};
    let blurred=add(Effect::Blur(14.),vec![BACKDROP]);
    let mut luma=[0.;20]; for i in 0..3 {for j in 0..3 {luma[i*4+j]=LUMA[i];}} luma[19]=1.;
    let l=add(Effect::Matrix(luma,false),vec![blurred]);
    let mut threshold=luma;
    for i in 0..3 {for j in 0..3 {threshold[i*4+j]/=1.-KNEE;}threshold[16+i]=-KNEE/(1.-KNEE);}
    let t=add(Effect::Matrix(threshold,true),vec![blurred]);
    let square=add(Effect::Arithmetic([1.,0.,0.,0.],false),vec![t,t]);
    // Factor l*t*t*(3-2*t) as t*t*(3*l-2*l*t), keeping the native tree small.
    let factor=add(Effect::Arithmetic([-2.,3.,0.,0.],false),vec![l,t]);
    let deep=add(Effect::Arithmetic([1.,0.,0.,0.],false),vec![square,factor]);
    let sat=if expanded {1.15}else{0.95};
    let mut base=[0.;20];
    for i in 0..3 {for j in 0..3 {base[i*4+j]=SPAN*(LUMA[i]*(1.-sat)+if i==j {sat}else{0.});} base[16+i]=LO;}
    base[19]=1.;
    let base=add(Effect::Matrix(base,false),vec![blurred]);
    let toned=add(Effect::Arithmetic([0.,1.,-SPAN*DEEP,0.],false),vec![base,deep]);
    add(Effect::Matrix(matrix([1.;3],0.),true),vec![toned]);
    nodes
}

/// Reference evaluation of the production graph; a constant input makes Gaussian blur an identity.
pub fn evaluate(nodes: &[Node], backdrop: [f32;4]) -> [f32;4] {
    let mut values: Vec<[f32;4]>=Vec::new();
    for node in nodes {
        let input=|i:usize|if node.inputs[i]==BACKDROP {backdrop}else{values[node.inputs[i]]};
        let out=match &node.effect {
            Effect::Blur(_) => input(0),
            Effect::Matrix(m,clamp) => {
                let c=input(0); let mut out=[0.;4];
                for j in 0..4 {out[j]=m[16+j]+(0..4).map(|i|c[i]*m[i*4+j]).sum::<f32>();if *clamp {out[j]=out[j].clamp(0.,1.);}} out
            }
            Effect::Arithmetic(k,clamp) => {
                let a=input(0);let b=input(1);let mut out=[0.;4];
                for j in 0..4 {out[j]=k[0]*a[j]*b[j]+k[1]*a[j]+k[2]*b[j]+k[3];if *clamp {out[j]=out[j].clamp(0.,1.);}} out
            }
        }; values.push(out);
    }
    values.last().copied().unwrap_or(backdrop)
}

// Port of GlassLayer::UpdateBlurredShape/NotchGeometry (mac-island 3.1.1).
#[allow(clippy::too_many_arguments)]
pub fn glass_path(canvas: f32,w: f32,h: f32,r: f32,gap: f32,notch: bool,curve: u32,scale: f32) -> Option<tiny_skia::Path> {
    let x=(canvas-w)/2.;
    if notch {crate::renderer::build_notch(x*scale,w*scale,h*scale,r*scale,crate::renderer::notch_shoulder(r,curve)*scale,true)}
    else {crate::renderer::build_rounded_rect(x*scale,gap*scale,w*scale,h*scale,r*scale)}
}

// Port of glass.js isleInner/isleNormal, used only for the drawn light (never reads a backdrop).
// `sh`: the notch's shoulder (build_notch); its curves bound the sides near the top edge, so the rim
// follows them instead of running down the body's side through the glass.
pub(crate) fn inner(x:f32,y:f32,w:f32,h:f32,r:f32,notch:bool,sh:f32) -> f32 {
    let r=r.min(w/2.).min(h/2.); let dd=sh.min((h-r).max(0.));
    // The shoulder as a quarter ellipse (`sh` along the edge, `dd` down the side) centred beside the
    // body; its first-order distance meets the side's `x` where the curve ends.
    let side=|x:f32| if notch && sh>0.01 && dd>0.01 && y<dd {
        let (u,v)=((x+sh)/sh,(y-dd)/dd); let n=(u*u+v*v).sqrt();
        (n-1.)*n/((u/sh).powi(2)+(v/dd).powi(2)).sqrt().max(1e-6)
    } else {x};
    let mut d=side(x).min(side(w-x)).min(h-y);
    if y>h-r {let cx=x.clamp(r,w-r);d=d.min(r-((x-cx).powi(2)+(y-(h-r)).powi(2)).sqrt());}
    if !notch {d=d.min(y);if y<r {let cx=x.clamp(r,w-r);d=d.min(r-((x-cx).powi(2)+(y-r).powi(2)).sqrt());}}
    d
}

#[cfg(test)]
mod notch_light_tests {
    use super::inner;
    #[test]
    fn the_rim_follows_the_notch_shoulders_not_the_body_side() {
        let (w, h, r, sh): (f32, f32, f32, f32) = (300., 34., 13., 14.6);
        let dd = sh.min(h - r);
        // On build_notch's shoulder curve (quad (-sh,0) → (0,0) → (0,dd)) the edge is near.
        for i in 0..=10 {
            let t = i as f32 / 10.;
            let (bx, by) = (-(1. - t) * (1. - t) * sh, t * t * dd);
            assert!(inner(bx, by, w, h, r, true, sh).abs() < 0.12 * sh, "t {t}");
        }
        // Where the body meets the shoulder, no edge any more (the old seam), and it meets the side below.
        assert!(inner(0., dd * 0.25, w, h, r, true, sh) > RIM_WIDTH_SEAM);
        assert!((inner(3., dd, w, h, r, true, sh) - 3.).abs() < 0.01);
        // The pill keeps its own edges.
        assert_eq!(inner(0.5, 17., w, h, r, false, 0.), 0.5);
    }
    const RIM_WIDTH_SEAM: f32 = 2.0;
}
