//! A clipped, scrollable body with stationary navigation and primary footer controls.
use super::*;
impl PillRenderer{
    #[allow(clippy::too_many_arguments)]
    pub(super) fn panel_body(&mut self,canvas:f32,x:f32,y:f32,w:f32,h:f32,active:TabPage,s:&IslandState){
        let natural=crate::layout::natural_height(s,h);
        if natural<=h+0.1{self.panel_contents(canvas,x,y,w,h,active,s);return}
        // Taller than the canvas when the content is (the control center's catalog), so none of it is lost.
        let tall=(((y+natural)*self.scale).ceil() as u32+2).max(self.framebuffer.height);
        let parent=std::mem::replace(&mut self.layer,Pixmap::new(self.framebuffer.width,tall).unwrap());
        let mut content=s.clone();content.panel_scroll=0.;
        self.panel_contents(canvas,x,y,w,natural,active,&content);
        let rendered=std::mem::replace(&mut self.layer,parent);
        let footer=crate::layout::footer_height(s);let prefix=crate::layout::scroll_top(s);let max=natural-h;let scroll=s.panel_scroll.clamp(0.,max);
        self.composite_region(&rendered,x,y+84.,w,prefix-84.,0.);
        self.composite_region(&rendered,x,y+prefix,w,(h-footer-prefix).max(0.),-scroll);
        if footer>0.{self.composite_region(&rendered,x,y+h-footer,w,footer,-max);}
        let track=(h-footer-prefix-10.).max(1.);let thumb=(track*track/(track+max)).max(18.).min(track);
        self.round(x+w-7.,y+prefix+5.+(track-thumb)*scroll/max,2.,thumb,1.,Color::from_rgba8(167,173,183,130));
    }
    fn composite_region(&mut self,source:&Pixmap,x:f32,y:f32,w:f32,h:f32,dy:f32){
        if h<=0.{return}let Some(path)=build_rounded_rect(x*self.scale,y*self.scale,w*self.scale,h*self.scale,0.)else{return};
        let Some(mut mask)=Mask::new(self.framebuffer.width,self.framebuffer.height)else{return};mask.fill_path(&path,FillRule::Winding,true,Transform::identity());
        self.layer.draw_pixmap(0,0,source.as_ref(),&PixmapPaint::default(),Transform::from_translate(0.,dy*self.scale),Some(&mask));
    }
}
