use dioxus::prelude::*;
use dioxus_router::prelude::*;

use super::menu::MENU_LIST;

// <span className="text-sm text-gray-500 sm:text-center dark:text-gray-400">© 2023 <a href="https://cheminfuse.com/" className="hover:underline">Cheminfuse</a>. All Rights Reserved.
// </span>
// <ul className="flex flex-wrap items-center mt-3 text-sm text-gray-500 dark:text-gray-400 sm:mt-0">
//     <li>
//         <a href="/" className="mr-4 hover:underline md:mr-6 ">About</a>
//     </li>
//     <li>
//         <a href="/tools" className="mr-4 hover:underline md:mr-6">Tools</a>
//     </li>
//     <li>
//         <a href="/privacy-policy" className="mr-4 hover:underline md:mr-6">Privacy Policy</a>
//     </li>
//     <li>
//         <a href="/blog" className="mr-4 hover:underline md:mr-6">Blog</a>
//     </li>
//     <li>
//         <a href="/bontact" className="hover:underline">Contact</a>
//     </li>
// </ul>

#[inline_props]
pub fn Footer(cx: &Scope) -> Element {
    render!(
        (rsx!(
            footer {
                class: "bottom-0 w-full h-20 x-[100] border-t border-gray-200 bg-white",
                span {
                    class: "text-sm text-gray-500 sm:text-center dark:text-gray-400",
                    "© 2023 "
                    a {
                        href: "https://xraytsubaki.com/",
                        class: "hover:underline",
                        "Ryuichi Shimogawa"
                    }
                    ". All Rights Reserved."
                }
                ul {
                    class: "flex flex-wrap items-center mt-3 text-sm text-gray-500 dark:text-gray-400 sm:mt-0",
                    MENU_LIST.iter().map(
                        |menu| {
                            render!(rsx!(
                                li {
                                    Link {
                                        class: "mr-4 hover:underline md:mr-6",
                                        to: menu.route.clone(),
                                        menu.title
                                    }
        
                                }
                            ))
                        }
                    )
        
        
        
                }
            }
        )
        )
    )
}
