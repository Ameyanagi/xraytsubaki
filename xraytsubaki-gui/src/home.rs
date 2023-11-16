use dioxus::prelude::*;

#[inline_props]
pub fn Home(cx: Scope) -> Element {
    cx.render(rsx! { div { "Home" } })
}
