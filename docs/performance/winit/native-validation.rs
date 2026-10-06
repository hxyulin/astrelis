use std::{collections::HashMap, error::Error, time::{Duration, Instant}};
use astrelis_winit::{*, astrelis::{Frame, Mesh, MeshRenderer, Vertex, wgpu}, winit::{window::{Window,WindowId}, dpi::{LogicalPosition,LogicalSize,PhysicalSize}, event::WindowEvent}};
struct Check { mode:String, ids:Vec<WindowId>, counts:HashMap<WindowId,u64>, snapshot:HashMap<WindowId,u64>, renderer:Option<MeshRenderer>, mesh:Option<Mesh>, stage:u8, prepare_requested:bool, submit_requested:bool, skip:bool, prepares:u64, closed:usize, exiting:bool, started:Instant }
fn boom(s:&str)->Box<dyn Error> { std::io::Error::other(s).into() }
impl Handler for Check {
 type Message=u8; type Error=Box<dyn Error>;
 fn resumed(&mut self,cx:&mut AppContext<'_,u8>)->Result<(),Self::Error> {
  for i in 0..2 { let id=cx.create_window(Window::default_attributes().with_window_level(astrelis_winit::winit::window::WindowLevel::AlwaysOnTop).with_title(format!("Astrelis native review {i}" )).with_inner_size(LogicalSize::new(360.,260.)).with_position(LogicalPosition::new(70.+390.*i as f64,100.)),SurfaceSettings::new().depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8))?; self.ids.push(id); self.counts.insert(id,0); }
  if self.mode=="resumed_error" { return Err(boom("resumed failure")) }
  let proxy=cx.proxy(); std::thread::spawn(move||{ for stage in 1..=10 { std::thread::sleep(Duration::from_millis(if stage==1 {1500} else {350})); if proxy.send_event(stage).is_err(){break} } }); Ok(())
 }
 fn window_event(&mut self,_cx:&mut AppContext<'_,u8>,id:WindowId,event:WindowEvent)->Result<(),Self::Error>{ if matches!(event,WindowEvent::Resized(_)|WindowEvent::Occluded(_)|WindowEvent::ScaleFactorChanged{..}){println!("native {id:?} {event:?}");} Ok(()) }
 fn window_created(&mut self,cx:&mut AppContext<'_,u8>,id:WindowId)->Result<(),Self::Error>{
  let w=cx.window(id).unwrap(); assert_eq!(w.window().is_visible(),Some(false));
  if self.mode=="created_error" { return Err(boom("created failure")) }
  if self.renderer.is_none(){self.renderer=Some(MeshRenderer::new(w.graphics()));self.mesh=Some(w.graphics().create_mesh(&[Vertex::new([0.,0.7,0.],[1.,0.,0.,1.]),Vertex::new([-0.7,-0.7,0.],[0.,1.,0.,1.]),Vertex::new([0.7,-0.7,0.],[0.,0.,1.,1.])],&[0,1,2])?);} Ok(())
 }
 fn prepare(&mut self,cx:&mut AppContext<'_,u8>,id:WindowId)->Result<PrepareAction,Self::Error>{
  self.prepares+=1; if self.mode=="prepare_error" {return Err(boom("prepare failure"))}
  if self.stage==2 && id==self.ids[0] && !self.prepare_requested {self.prepare_requested=true;cx.request_redraw(id)?;}
  if self.stage==9 && id==self.ids[0] {cx.close_window(id)?; assert!(cx.window(id).is_none()); return Ok(PrepareAction::Render)}
  if self.skip {return Ok(PrepareAction::Skip)}
  self.renderer.as_mut().unwrap().prepare(cx.window(id).unwrap().render_format().unwrap())?; Ok(PrepareAction::Render)
 }
 fn render(&mut self,window:WindowInfo<'_>,frame:&mut Frame<'_,'static>)->Result<(),Self::Error>{
  {let mut pass=frame.render_pass().begin()?;self.renderer.as_mut().unwrap().draw(&mut pass,self.mesh.as_ref().unwrap())?;}
  if self.mode=="render_error" {return Err(boom("render failure"))}
  *self.counts.get_mut(&window.id()).unwrap()+=1; Ok(())
 }
 fn submitted(&mut self,cx:&mut AppContext<'_,u8>,id:WindowId,_submission:wgpu::SubmissionIndex)->Result<(),Self::Error>{
  if self.mode=="submitted_error" {return Err(boom("submitted failure"))}
  if self.stage==2 && id==self.ids[0] && !self.submit_requested {self.submit_requested=true;cx.request_redraw(id)?;} Ok(())
 }
 fn user_event(&mut self,cx:&mut AppContext<'_,u8>,stage:u8)->Result<(),Self::Error>{
  if self.mode=="shutdown_error" || self.mode=="closed_error" {cx.exit();return Ok(())}
  self.stage=stage; let a=self.ids[0];let b=self.ids[1];
  println!("stage={stage} counts={:?} prepares={} t={:.3}",self.counts,self.prepares,self.started.elapsed().as_secs_f64());
  match stage {
   1=>{assert!(self.counts.values().all(|c|*c>0));self.snapshot=self.counts.clone();},
   2=>{assert_eq!(self.counts,self.snapshot,"idle redraws");cx.request_redraw(a)?;},
   3=>{assert!(self.counts[&a]>=self.snapshot[&a]+2);assert_eq!(self.counts[&b],self.snapshot[&b]);
    let w=cx.window_mut(a).unwrap();let metrics=w.metrics(); assert_eq!(metrics.scale_factor(),w.window().scale_factor()); assert!(w.target().unwrap().supported_sample_counts().contains(&4));
    let update=w.process_event(&WindowEvent::Resized(metrics.physical_size()))?;assert!(!update.metrics_changed);assert!(update.redraw_requested);
    w.process_event(&WindowEvent::Resized(PhysicalSize::new(0,0)))?;assert!(!w.can_present());assert!(matches!(w.begin_frame(),Err(astrelis::FrameError::Suspended)));
    w.process_event(&WindowEvent::Resized(metrics.physical_size()))?;w.set_sample_count(4)?;
    assert_eq!(w.render_format().unwrap().sample_count,4);w.set_visible(false);self.snapshot=self.counts.clone();cx.request_redraw_at(a,Instant::now()+Duration::from_millis(20))?;},
   4=>{assert_eq!(self.counts[&a],self.snapshot[&a],"hidden window drew");let w=cx.window_mut(a).unwrap();let generation=w.surface_generation();
    w.suspend();w.suspend();assert!(w.render_format().is_none());assert!(w.target().is_none());assert!(w.set_sample_count(3).is_err());w.set_sample_count(1)?;w.set_sample_count(4)?;w.resume()?;w.resume()?;
    assert_eq!(w.surface_generation(),generation+1);let t=w.target().unwrap();assert_eq!(t.sample_count(),4);assert_eq!(t.depth_stencil_format(),Some(wgpu::TextureFormat::Depth24PlusStencil8));w.set_visible(true);},
   5=>{assert!(self.counts[&a]>self.snapshot[&a]);self.snapshot=self.counts.clone();self.skip=true;cx.set_redraw_mode(a,RedrawMode::Continuous)?;},
   6=>{assert_eq!(self.counts,self.snapshot,"Skip must not present");self.snapshot.insert(a,self.prepares);},
   7=>{assert_eq!(self.prepares,self.snapshot[&a],"Skip continuous spun");self.skip=false;cx.set_redraw_mode(a,RedrawMode::OnDemand)?;self.snapshot=self.counts.clone();cx.request_redraw(a)?;cx.request_redraw_at(a,Instant::now()+Duration::from_millis(20))?;cx.request_redraw_at(a,Instant::now()+Duration::from_millis(120))?;},
   8=>{assert!(self.counts[&a]>=self.snapshot[&a]+2,"deadline lost across immediate redraw");assert_eq!(self.counts[&b],self.snapshot[&b]);cx.set_redraw_mode(a,RedrawMode::Continuous)?;},
   9=>{assert!(self.counts[&a]>self.snapshot[&a]+2);cx.set_redraw_mode(a,RedrawMode::OnDemand)?;cx.request_redraw(a)?;},
   10=>{assert_eq!(self.closed,1);assert!(cx.window(a).is_none());cx.close_window(b)?;assert!(cx.window(b).is_none());}, _=>{}
  } Ok(())
 }
 fn window_closed(&mut self,cx:&mut AppContext<'_,u8>,id:WindowId)->Result<(),Self::Error>{assert!(cx.window(id).is_none());self.closed+=1;if self.mode=="closed_error" {Err(boom("closed failure"))} else {Ok(())}}
 fn exiting(&mut self,_cx:&mut AppContext<'_,u8>)->Result<(),Self::Error>{assert_eq!(self.closed,2);self.exiting=true;if self.mode!="success"{Err(boom("exiting failure"))}else{Ok(())}}
}
fn main()->Result<(),Box<dyn Error>> {
 let mode=std::env::args().nth(1).unwrap_or("success".into());let mut check=Check{mode:mode.clone(),ids:vec![],counts:HashMap::new(),snapshot:HashMap::new(),renderer:None,mesh:None,stage:0,prepare_requested:false,submit_requested:false,skip:false,prepares:0,closed:0,exiting:false,started:Instant::now()};
 let result=Runner::new()?.run(&mut check);assert!(check.exiting);assert_eq!(check.closed,2);
 match mode.as_str(){"success"=>{result?;assert_eq!(check.stage,10);},"render_error"=>{assert!(matches!(result,Err(RunError::Handler{callback:Callback::Render,..})));assert_eq!(check.counts.values().sum::<u64>(),0);},"submitted_error"=>{assert!(matches!(result,Err(RunError::Handler{callback:Callback::Submitted,..})));assert_eq!(check.counts.values().sum::<u64>(),1);},"created_error"=>assert!(matches!(result,Err(RunError::Handler{callback:Callback::WindowCreated,..}))),"resumed_error"=>assert!(matches!(result,Err(RunError::Handler{callback:Callback::Resumed,..}))),"prepare_error"=>assert!(matches!(result,Err(RunError::Handler{callback:Callback::Prepare,..}))),"closed_error"=>assert!(matches!(result,Err(RunError::Handler{callback:Callback::WindowClosed,..}))),"shutdown_error"=>assert!(matches!(result,Err(RunError::Handler{callback:Callback::Exiting,..}))),_=>panic!("unknown mode")}
 println!("PASS {mode} closed={} exiting={}",check.closed,check.exiting);Ok(())
}
