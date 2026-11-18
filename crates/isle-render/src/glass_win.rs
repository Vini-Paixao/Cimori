// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 caliberda (https://github.com/cesarkali), 2026 zickrian
//! Port of GlassCompositionParts/GlassLayer::ShowBlurred/GlassPath/GlassEffect (mac-island 3.1.1).
//! isle's tone expression is translated to the same static Direct2D effect graph; no capture or lens.
use crate::glass::{self, Effect};
use std::{cell::RefCell, rc::Rc};
use windows::{core::{implement, Interface, HSTRING, PCWSTR, GUID, w},
    Foundation::{IPropertyValue, PropertyValue, Numerics::Vector2},
    Graphics::{IGeometrySource2D, IGeometrySource2D_Impl, Effects::*},
    System::{DispatcherQueue, DispatcherQueueController},
    UI::{Composition::{*, Desktop::DesktopWindowTarget}, ViewManagement::UISettings},
    Win32::{Foundation::*, Graphics::{Direct2D::{Common::*,*}, Dwm::*},
        System::{LibraryLoader::GetModuleHandleW, WinRT::{*, Composition::ICompositorDesktopInterop, Graphics::Direct2D::*}},
        UI::WindowsAndMessaging::*}};

#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct GlassEffect { effect: Effect, inputs: Vec<IGraphicsEffectSource>, name: RefCell<HSTRING> }
impl IGraphicsEffectSource_Impl for GlassEffect_Impl {}
impl IGraphicsEffect_Impl for GlassEffect_Impl {
    fn Name(&self) -> windows::core::Result<HSTRING> {Ok(self.name.borrow().clone())}
    fn SetName(&self,name:&HSTRING) -> windows::core::Result<()> {*self.name.borrow_mut()=name.clone();Ok(())}
}
impl IGraphicsEffectD2D1Interop_Impl for GlassEffect_Impl {
    fn GetEffectId(&self)->windows::core::Result<GUID> {Ok(match self.effect {Effect::Blur(_)=>CLSID_D2D1GaussianBlur,Effect::Matrix(..)=>CLSID_D2D1ColorMatrix,Effect::Arithmetic(..)=>CLSID_D2D1ArithmeticComposite})}
    fn GetNamedPropertyMapping(&self,_:&PCWSTR,_:*mut u32,_:*mut GRAPHICS_EFFECT_PROPERTY_MAPPING)->windows::core::Result<()> {Err(E_INVALIDARG.into())}
    fn GetPropertyCount(&self)->windows::core::Result<u32> {Ok(if matches!(self.effect,Effect::Arithmetic(..)){2}else{3})}
    fn GetProperty(&self,index:u32)->windows::core::Result<IPropertyValue> {
        let p=match (&self.effect,index) {
            (Effect::Blur(s),0)=>PropertyValue::CreateSingle(*s)?,
            (Effect::Blur(_),1|2)=>PropertyValue::CreateUInt32(1)?, // balanced blur, hard edge
            (Effect::Matrix(m,_),0)=>PropertyValue::CreateSingleArray(m)?,
            // isle's graph works directly on RGB; the last matrix restores alpha after subtraction.
            // PREMULTIPLIED would unpremultiply that RGB and incorrectly brighten the backdrop.
            (Effect::Matrix(..),1)=>PropertyValue::CreateUInt32(2)?, // D2D1_COLORMATRIX_ALPHA_MODE_STRAIGHT
            (Effect::Matrix(_,clamp),2)=>PropertyValue::CreateBoolean(*clamp)?,
            (Effect::Arithmetic(k,_),0)=>PropertyValue::CreateSingleArray(k)?,
            (Effect::Arithmetic(_,clamp),1)=>PropertyValue::CreateBoolean(*clamp)?,
            _=>return Err(E_BOUNDS.into()),
        };p.cast()
    }
    fn GetSource(&self,index:u32)->windows::core::Result<IGraphicsEffectSource> {self.inputs.get(index as usize).cloned().ok_or_else(||E_BOUNDS.into())}
    fn GetSourceCount(&self)->windows::core::Result<u32> {Ok(self.inputs.len() as u32)}
}

#[implement(IGeometrySource2D, IGeometrySource2DInterop)]
struct GlassPath {geometry: ID2D1Geometry}
impl IGeometrySource2D_Impl for GlassPath_Impl {}
impl IGeometrySource2DInterop_Impl for GlassPath_Impl {
    fn GetGeometry(&self)->windows::core::Result<ID2D1Geometry> {Ok(self.geometry.clone())}
    fn TryGetGeometryUsingFactory(&self,_:Option<&ID2D1Factory>)->windows::core::Result<ID2D1Geometry> {Err(E_NOTIMPL.into())}
}

struct Apartment;
impl Drop for Apartment {fn drop(&mut self){unsafe{RoUninitialize();}}}
struct Context {
    compositor: Compositor, factory: ID2D1Factory, brushes: [CompositionEffectBrush;2],
    _queue: Option<DispatcherQueueController>, _apartment: Apartment,
}
thread_local! {static CONTEXT: RefCell<Option<Rc<Context>>> = const {RefCell::new(None)};}
impl Context {
    // Port of GlassCompositionParts/GlassLayer::Backdrop (mac-island 3.1.1).
    fn get()->windows::core::Result<Rc<Self>> {
        CONTEXT.with(|slot| {
            if let Some(c)=slot.borrow().as_ref(){return Ok(c.clone());}
            unsafe {RoInitialize(RO_INIT_SINGLETHREADED)?;}
            let apartment=Apartment;
            let queue=if DispatcherQueue::GetForCurrentThread().is_ok(){None}else{Some(unsafe{CreateDispatcherQueueController(DispatcherQueueOptions{
                dwSize:std::mem::size_of::<DispatcherQueueOptions>() as u32,threadType:DQTYPE_THREAD_CURRENT,apartmentType:DQTAT_COM_STA})?})};
            let compositor=Compositor::new()?;
            let factory=unsafe{D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED,None)?};
            let brush=|expanded|->windows::core::Result<CompositionEffectBrush> {
                // Composition accepts trees, not shared effect nodes. Expand references into fresh
                // nodes; each leaf still names the same single backdrop source brush.
                fn tree(nodes:&[glass::Node],index:usize)->windows::core::Result<IGraphicsEffectSource> {
                    let node=&nodes[index];
                    // HostBackdropBrush already supplies Windows' own blur. Extra Gaussian passes
                    // on each tree branch exceed Composition's complexity limit and are unnecessary.
                    if matches!(node.effect,Effect::Blur(_)) {return CompositionEffectSourceParameter::Create(&HSTRING::from("backdrop"))?.cast();}
                    let inputs=node.inputs.iter().map(|&i|if i==glass::BACKDROP {
                        CompositionEffectSourceParameter::Create(&HSTRING::from("backdrop"))?.cast()
                    } else {tree(nodes,i)}).collect::<windows::core::Result<Vec<_>>>()?;
                    let effect:IGraphicsEffect=GlassEffect{effect:node.effect.clone(),inputs,name:RefCell::new(HSTRING::new())}.into();
                    effect.cast()
                }
                let nodes=glass::tone_graph(expanded);let top:IGraphicsEffect=tree(&nodes,nodes.len()-1)?.cast()?;
                let brush=compositor.CreateEffectFactory(&top)?.CreateBrush()?;
                brush.SetSourceParameter(&HSTRING::from("backdrop"),&compositor.CreateHostBackdropBrush()?)?;
                Ok(brush)
            };
            let brushes=[brush(false)?,brush(true)?];
            let context=Rc::new(Self{compositor,factory,brushes,_queue:queue,_apartment:apartment});
            *slot.borrow_mut()=Some(context.clone());Ok(context)
        })
    }
    // Port of GlassPath/NotchGeometry (mac-island 3.1.1), sharing the renderer's exact path.
    fn clip(&self,path:&tiny_skia::Path)->windows::core::Result<CompositionGeometricClip> {
        unsafe {
            let geometry=self.factory.CreatePathGeometry()?;let sink=geometry.Open()?;
            sink.SetFillMode(D2D1_FILL_MODE_WINDING);
            let point=|p:tiny_skia::Point|D2D_POINT_2F{x:p.x,y:p.y};
            for part in path.segments() {match part {
                tiny_skia::PathSegment::MoveTo(p)=>sink.BeginFigure(point(p),D2D1_FIGURE_BEGIN_FILLED),
                tiny_skia::PathSegment::LineTo(p)=>sink.AddLine(point(p)),
                tiny_skia::PathSegment::QuadTo(p,q)=>sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT{point1:point(p),point2:point(q)}),
                tiny_skia::PathSegment::CubicTo(p,q,r)=>sink.AddBezier(&D2D1_BEZIER_SEGMENT{point1:point(p),point2:point(q),point3:point(r)}),
                tiny_skia::PathSegment::Close=>sink.EndFigure(D2D1_FIGURE_END_CLOSED),
            }}sink.Close()?;
            let source:IGeometrySource2D=GlassPath{geometry:geometry.cast()?}.into();
            let path=CompositionPath::Create(&source)?;
            self.compositor.CreateGeometricClipWithGeometry(&self.compositor.CreatePathGeometryWithPath(&path)?)
        }
    }
}

struct Parts {context:Rc<Context>,target:DesktopWindowTarget,sprite:SpriteVisual,hwnd:HWND,path:Option<tiny_skia::Path>,rect:RECT,expanded:Option<bool>}
impl Drop for Parts {fn drop(&mut self){let _=self.target.SetRoot(None);unsafe{let _=DestroyWindow(self.hwnd);}}}
#[derive(Default)]
pub(crate) struct Backdrop {parts:Option<Parts>,failed:bool,visible:bool,wanted:bool}
unsafe extern "system" fn wnd_proc(hwnd:HWND,msg:u32,wp:WPARAM,lp:LPARAM)->LRESULT {
    match msg {WM_NCHITTEST=>LRESULT(HTTRANSPARENT as isize),WM_MOUSEACTIVATE=>LRESULT(MA_NOACTIVATE as isize),_=>unsafe{DefWindowProcW(hwnd,msg,wp,lp)}}
}
impl Backdrop {
    // Port of GlassLayer::ShowBlurred (mac-island 3.1.1); DWM does all ongoing backdrop work.
    fn create()->windows::core::Result<Parts> {
        let context=Context::get()?;
        unsafe {
            let instance=GetModuleHandleW(None)?;
            let class=w!("CimoriGlassBackdrop");
            let wc=WNDCLASSW{lpfnWndProc:Some(wnd_proc),hInstance:instance.into(),lpszClassName:class,..Default::default()};
            RegisterClassW(&wc);
            let hwnd=CreateWindowExW(WS_EX_NOREDIRECTIONBITMAP|WS_EX_LAYERED|WS_EX_TRANSPARENT|WS_EX_TOPMOST|WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE,class,w!(""),WS_POPUP,0,0,0,0,None,None,instance,None)?;
            let made=(||->windows::core::Result<Parts>{
                SetLayeredWindowAttributes(hwnd,COLORREF(0),255,LWA_ALPHA)?;
                let host=BOOL(1);DwmSetWindowAttribute(hwnd,DWMWINDOWATTRIBUTE(17),&host as *const _ as *const _,std::mem::size_of::<BOOL>() as u32)?;
                let target=context.compositor.cast::<ICompositorDesktopInterop>()?.CreateDesktopWindowTarget(hwnd,true)?;
                let sprite=context.compositor.CreateSpriteVisual()?;target.SetRoot(&sprite)?;
                Ok(Parts{context,target,sprite,hwnd,path:None,rect:RECT::default(),expanded:None})
            })();
            if made.is_err(){let _=DestroyWindow(hwnd);}made
        }
    }
    pub fn set_visible(&mut self,on:bool) {
        if self.visible!=on {if let Some(p)=&self.parts {unsafe{let _=ShowWindow(p.hwnd,if on{SW_SHOWNOACTIVATE}else{SW_HIDE});}}self.visible=on;}
    }
    pub fn disable(&mut self) {self.wanted=false;self.set_visible(false);}
    pub fn restore_visibility(&mut self,visible:bool) {self.set_visible(visible && self.wanted && !self.failed);}
    pub fn update(&mut self,owner:HWND,path:&tiny_skia::Path,expanded:bool,enabled:bool)->bool {
        self.wanted=enabled;
        if !enabled || self.failed {self.set_visible(false);return false;}
        if !UISettings::new().and_then(|s|s.AdvancedEffectsEnabled()).unwrap_or(true) {self.set_visible(false);return false;}
        let done=(||->windows::core::Result<()> {
            if self.parts.is_none(){self.parts=Some(Self::create()?);}
            let p=self.parts.as_mut().unwrap();
            if p.path.as_ref()!=Some(path) {p.sprite.SetClip(&p.context.clip(path)?)?;p.path=Some(path.clone());}
            if p.expanded!=Some(expanded) {p.sprite.SetBrush(&p.context.brushes[usize::from(expanded)])?;p.expanded=Some(expanded);}
            unsafe {
                let mut rect=RECT::default();GetWindowRect(owner,&mut rect)?;
                if p.rect!=rect || !self.visible || GetWindow(owner,GW_HWNDNEXT).ok()!=Some(p.hwnd) {
                    p.sprite.SetSize(Vector2{X:(rect.right-rect.left) as f32,Y:(rect.bottom-rect.top) as f32})?;
                    SetWindowPos(p.hwnd,owner,rect.left,rect.top,rect.right-rect.left,rect.bottom-rect.top,SWP_NOACTIVATE|SWP_SHOWWINDOW)?;
                    p.rect=rect;self.visible=true;
                }
            }Ok(())
        })();
        if let Err(error)=done {tracing::warn!("Liquid Glass: {error}; usando fundo sólido");self.set_visible(false);self.parts=None;self.failed=true;return false;}true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tone_matrices_do_not_unpremultiply_the_subtracted_alpha() {
        let matrix=glass::tone_graph(false).last().unwrap().effect.clone();
        let effect:IGraphicsEffect=GlassEffect{effect:matrix,inputs:Vec::new(),name:RefCell::new(HSTRING::new())}.into();
        let interop=effect.cast::<IGraphicsEffectD2D1Interop>().unwrap();
        let alpha=unsafe{interop.GetProperty(1)}.unwrap().GetUInt32().unwrap();
        assert_eq!(alpha,2,"raw graph math requires STRAIGHT; PREMULTIPLIED would divide RGB by 0.6672 before restoring alpha");
    }
    #[test]
    fn native_compositor_accepts_the_tone_graph_and_both_shape_clips() {
        let context=Context::get().expect("native composition/effect graph");
        for notch in [false,true] {
            let path=glass::glass_path(720.,400.,260.,28.,6.,notch,75,1.5).unwrap();
            context.clip(&path).expect("shared native path clip");
        }
    }
}
