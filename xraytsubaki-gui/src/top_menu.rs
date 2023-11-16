use crate::TOP_DIR;
use dioxus::{prelude::{GlobalAttributes, *}, html::div};


pub fn TopMenu(cx: Scope) -> Element {
    render!(
        rsx!(
            nav{
                button{
                    class : "flex inline-flex flex-wrap items-center text-sm font-semibold leading-6 text-gray-900 bg-slate-800 text-slate-200 gap-x-1",
                    aria_expanded: "false",
                    r#type: "button",

                }
                

                
        
                
        
            }
        )
    )
}
