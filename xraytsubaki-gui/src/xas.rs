use dioxus::prelude::*;

#[inline_props]
pub fn XAS(cx: Scope) -> Element {
    cx.render(rsx! { div { "XAS" } })
}
