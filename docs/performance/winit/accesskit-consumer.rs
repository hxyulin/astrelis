#![allow(dead_code)]
use astrelis_winit::{AppContext,Handler,WindowInfo,astrelis::Frame,winit::{event_loop::ActiveEventLoop,window::Window}};
enum Message { Accessibility(accesskit_winit::Event) }
impl From<accesskit_winit::Event> for Message { fn from(event:accesskit_winit::Event)->Self{Self::Accessibility(event)} }
struct BoxedErrorHandler;
impl Handler for BoxedErrorHandler {
 type Message=Message; type Error=Box<dyn std::error::Error>;
 fn render(&mut self,_:WindowInfo<'_>,_:&mut Frame<'_,'static>)->Result<(),Self::Error>{Ok(())}
}
fn native_adapter(ctx:&AppContext<'_,Message>,events:&ActiveEventLoop,window:&Window){
 let _adapter=accesskit_winit::Adapter::with_event_loop_proxy(events,window,ctx.proxy());
}
