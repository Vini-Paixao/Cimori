// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian, 2026 caliberda (https://github.com/cesarkali)
//! Port of isle @ a29a7c4 glass.js light pass, and mac-island 3.1.1 GlassFill/GlassCardLight.
use super::*;
use crate::glass::{inner,SHEEN,GLOW,REACH,RIM,RIM_WIDTH};
/// How far in the glow reaches (6 of its reaches); the rim stops sooner (RIM_WIDTH x 8).
const FAR:f32=REACH*6.;

fn sheen_at(yy:f32,h:f32)->f32 {SHEEN*(1.-(yy/h.max(1.)).clamp(0.,1.))}

/// The light's alpha at a point of the pill (its own coordinates), for the pixel's `coverage`.
#[allow(clippy::too_many_arguments)]
fn light_at(xx:f32,yy:f32,w:f32,h:f32,r:f32,notch:bool,sh:f32,coverage:f32)->u8 {
    let light_len=(1.0f32+1.25*1.25).sqrt();
    let e=inner(xx,yy,w,h,r,notch,sh);
    let vertical=(yy/h.max(1.)).clamp(0.,1.);
    let mut amount=sheen_at(yy,h);
    // The original's inner glow leaves out the free top; the rim includes it.
    // (Past FAR the glow adds under 0.03 of a 255th: not worth its exp.)
    let glow_edge=inner(xx,yy,w,h,r,true,sh);
    if (0.0..FAR).contains(&glow_edge) {amount+=GLOW*(-glow_edge/REACH).exp()*(0.3+0.7*vertical);}
    if e>=0. && e<RIM_WIDTH*8. {
        let dx=inner(xx+1.,yy,w,h,r,notch,sh)-inner(xx-1.,yy,w,h,r,notch,sh);
        let dy=inner(xx,yy+1.,w,h,r,notch,sh)-inner(xx,yy-1.,w,h,r,notch,sh);
        let len=(dx*dx+dy*dy).sqrt().max(0.00001);
        let facing=(dx+dy*1.25)/(len*light_len);
        let lit=facing.max(0.).powf(1.6)+0.6*(-facing).max(0.).powf(1.6);
        amount+=RIM*(0.22+lit)*((-e/RIM_WIDTH).exp()+0.3*(-e/(RIM_WIDTH*3.5)).exp());
    }
    (amount.clamp(0.,1.)*coverage*255.).round() as u8
}

impl PillRenderer {
    // Port of glass.js sheen/inner glow/isleNormal/lit rim; cached light only, no backdrop pixels.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn glass_light(&mut self,path:&tiny_skia::Path,x:f32,y:f32,w:f32,h:f32,r:f32,notch:bool,sh:f32) {
        let key=vec![x.to_bits(),y.to_bits(),w.to_bits(),h.to_bits(),r.to_bits(),self.scale.to_bits(),u32::from(notch),sh.to_bits()];
        let (pw,ph)=(self.layer.width() as usize,self.layer.height() as usize);
        // Only the pill's bounds: the rest of the canvas has no light (and cost ~13 ms a moving frame at 150%).
        let b=path.bounds();
        let (x0,y0)=((b.left().floor().max(0.) as usize).min(pw),(b.top().floor().max(0.) as usize).min(ph));
        let (x1,y1)=((b.right().ceil().max(0.) as usize).min(pw),(b.bottom().ceil().max(0.) as usize).min(ph));
        if self.glass_optics.as_ref().is_none_or(|(k,_)|*k!=key) {
            let mut light=vec![0u8;(x1-x0)*(y1-y0)];
            // The pill's coverage over its bounds only, not the whole canvas.
            let (mw,mh)=((x1-x0).max(1),(y1-y0).max(1));
            let mut mask=Mask::new(mw as u32,mh as u32).unwrap();
            mask.fill_path(path,FillRule::Winding,true,Transform::from_translate(-(x0 as f32),-(y0 as f32)));
            let scale=self.scale;
            // Far from every edge (past the glow's reach, so past the rim too) the light is the sheen
            // alone, one value a row: those runs are filled at once (an opening panel's frames at 150 %
            // spent ~13 ms here otherwise).
            let far=FAR;
            for py in y0..y1 {
                let row=&mut light[(py-y0)*(x1-x0)..(py-y0+1)*(x1-x0)];
                let yy=(py as f32+0.5)/scale-if notch {0.}else{y};
                let inside_rows=h-yy>=far && (notch || yy>=far);
                let (from,to)=if inside_rows {
                    (((x+far)*scale-0.5).ceil().max(x0 as f32) as usize,(((x+w-far)*scale-0.5).floor()+1.).min(x1 as f32).max(0.) as usize)
                } else {(x1,x1)};
                let sheen=(sheen_at(yy,h).clamp(0.,1.)*255.).round() as u8;
                for px in x0..x1 {
                    if px>=from && px<to {row[px-x0]=sheen;continue;}
                    let coverage=mask.data()[(py-y0)*mw+px-x0] as f32/255.;if coverage==0. {continue;}
                    let xx=(px as f32+0.5)/scale-x;
                    row[px-x0]=light_at(xx,yy,w,h,r,notch,sh,coverage);
                }
            }
            self.glass_optics=Some((key,light));
        }
        // White light at `alpha`, over the layer (premultiplied source-over), in the bounds only.
        let light=&self.glass_optics.as_ref().unwrap().1;
        let div255=|v:u32|(v+128+((v+128)>>8))>>8;
        let pixels=self.layer.data_mut();
        for py in y0..y1 {
            let row=&light[(py-y0)*(x1-x0)..(py-y0+1)*(x1-x0)];
            for (dst,&a) in pixels[(py*pw+x0)*4..(py*pw+x1)*4].chunks_exact_mut(4).zip(row) {
                if a==0 {continue;}
                let keep=255-a as u32;
                for c in dst.iter_mut() {*c=(a as u32+div255(*c as u32*keep)).min(255) as u8;}
            }
        }
    }
    // Port of GlassFill/MakeGlassCardBrush (mac-island 3.1.1) and isle stylesheet.css glass surfaces.
    pub(super) fn glass_card(&mut self,x:f32,y:f32,w:f32,h:f32,r:f32) {
        let s=self.scale;
        let Some(path)=build_rounded_rect(x*s,y*s,w*s,h*s,r*s) else{return};
        let mut paint=Paint{anti_alias:true,..Default::default()};
        paint.set_color_rgba8(255,255,255,15);self.layer.fill_path(&path,&paint,FillRule::Winding,Transform::identity(),None);
        paint.shader=tiny_skia::LinearGradient::new(tiny_skia::Point::from_xy(x*s,y*s),tiny_skia::Point::from_xy(x*s,(y+h)*s),
            vec![tiny_skia::GradientStop::new(0.,Color::from_rgba8(255,255,255,28)),tiny_skia::GradientStop::new(0.5,Color::from_rgba8(255,255,255,9)),tiny_skia::GradientStop::new(1.,Color::TRANSPARENT)],tiny_skia::SpreadMode::Pad,Transform::identity()).unwrap();
        self.layer.fill_path(&path,&paint,FillRule::Winding,Transform::identity(),None);
        paint.shader=tiny_skia::LinearGradient::new(tiny_skia::Point::from_xy(x*s,y*s),tiny_skia::Point::from_xy((x+w)*s,(y+h)*s),
            vec![tiny_skia::GradientStop::new(0.,Color::from_rgba8(255,255,255,80)),tiny_skia::GradientStop::new(0.3,Color::from_rgba8(255,255,255,24)),tiny_skia::GradientStop::new(0.6,Color::from_rgba8(255,255,255,12)),tiny_skia::GradientStop::new(1.,Color::from_rgba8(255,255,255,40))],tiny_skia::SpreadMode::Pad,Transform::identity()).unwrap();
        self.layer.stroke_path(&path,&paint,&Stroke{width:s,..Default::default()},Transform::identity(),None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_geometry_reuses_the_light_raster() {
        let mut renderer=PillRenderer::new(720,520);let mut cfg=IsleConfig::default();cfg.liquid_glass=true;
        let geom=PillGeometry::default();let state=IslandState::default();
        renderer.render(&geom,&state,&cfg);
        let first=renderer.glass_optics.as_ref().unwrap().1.as_ptr();
        renderer.render(&geom,&state,&cfg);
        assert_eq!(first,renderer.glass_optics.as_ref().unwrap().1.as_ptr());
    }
}

#[cfg(test)]
mod light_tests {
    use super::*;
    #[test]
    fn the_far_inside_is_the_sheen_alone_as_the_full_light_says() {
        let far=FAR;
        for (w,h,r,notch,sh) in [(600.,470.,32.,false,0.),(600.,470.,32.,true,22.),(300.,120.,18.,true,14.6)] {
            let mut yy=if notch {far} else {far}+0.25;
            while h-yy>=far {
                let mut xx=far+0.25;
                while w-xx>=far {
                    assert_eq!(light_at(xx,yy,w,h,r,notch,sh,1.),(sheen_at(yy,h).clamp(0.,1.)*255.).round() as u8,"{xx},{yy}");
                    xx+=7.;
                }
                yy+=5.;
            }
        }
    }
}
