use crate::Route;
use dioxus::{html::nav, prelude::*};
use dioxus_router::prelude::*;

pub struct MenuContent {
    pub title: &'static str,
    pub route: Route,
}

pub const MENU_LIST: &[MenuContent] = &[
    MenuContent {
        title: "About",
        route: Route::Home {},
    },
    MenuContent {
        title: "Tools",
        route: Route::Home {},
    },
    MenuContent {
        title: "Blog",
        route: Route::Home {},
    },
    MenuContent {
        title: "Contact",
        route: Route::Home {},
    },
];

pub fn Navibar(cx: Scope) -> Element {
    let nav_hid = use_state(cx, || false);

    render! {
        nav { class: "w-full h-20 x-[100] border-b border-gray-200 bg-white",
            div { class: "flex items-center justify-between w-full h-full px-2 2xl:px-16",
                // main menu
                div { class: "hidden md:flex",
                    div { class: "flex items-center justify-center gap-2 md:gap-8",
                        ul {
                            style: "color: `$(linkColor)`",
                            class: "flex items-center justify-center gap-2 uppercase md:gap-8",

                            MENU_LIST.iter().map(|menu| {
                                rsx! {
                                    Link { to: menu.route.clone(), li { class: "ml-10 text-xl hover:border-b", menu.title } }
                                }
                            })
                        }
                    }
                }

                // Hidden menu
                //     <div className={nav ? "md:hidden fixed left-0 top-0 w-full h-[100vh] bg-[#818a91]/70 z-50" : ""}>
                //     <div className={nav
                //         ? "fixed left-0 top-0 w-[75%] sm:w-[45%] h-screen bg-white p-10 ease-in duration-200 shadow-l shadow-xl shadow-black"
                //         : "fixed left-[-200%] top-0 p-10 ease-in duration-200 shadow-l shadow-xl shadow-black"}>

                //         <div className="flex items-center justify-between w-full">
                //             <Image src={"/images/logo.png"} width={200} height={200} alt="" />
                //             <div onClick={handleNav} className="p-3 rounded-full shadow-lg cursor-pointer shadow-gray-400">
                //                 <AiOutlineClose />
                //             </div>
                //         </div>

                //         <div className="my-4 border-b border-gray-300">
                //         </div>

                //         <div className="flex flex-col py-4">
                //             <ul className="uppercase">
                //                 <Link onClick={handleNav} href="/"><li className="py-4 text-xl hover:border-b">About</li></Link>
                //                 <Link onClick={handleNav} href="/tools"><li className="py-4 text-xl hover:border-b">Tools</li></Link>
                //                 <Link onClick={handleNav} href="/blog/1"><li className="py-4 text-xl hover:border-b">Blog</li></Link>
                //                 <Link onClick={handleNav} href="/contact"><li className="py-4 text-xl hover:border-b">Contact</li></Link>
                //             </ul>
                //             <div className="pt-40">
                //                 <p className="tracking-widest uppercase">Let's Connect</p>
                //                 <div className="flex items-center justify-between my-4 w-full sm:w-[80%]">
                //                     {/* <div className="p-3 rounded-full shadow-lg cursor-pointer shadow-gray-400 hover:scale-105 ease-ind duration-10">
                //                         <FaLinkedinIn />
                //                     </div> */}
                //                     <div className="p-3 rounded-full shadow-lg cursor-pointer shadow-gray-400 hover:scale-105 ease-ind duration-10">
                //                         <Link
                //                             href="https://github.com/Ameyanagi"
                //                         >
                //                             <FaGithub />
                //                         </Link>
                //                     </div>
                //                     {/* <div className="p-3 rounded-full shadow-lg cursor-pointer shadow-gray-400 hover:scale-105 ease-ind duration-10">
                //                         <AiOutlineMail />
                //                     </div>
                //                     <div className="p-3 rounded-full shadow-lg cursor-pointer shadow-gray-400 hover:scale-105 ease-ind duration-10">
                //                         <BsFillPersonLinesFill />
                //                     </div> */}

                //                 </div>

                //             </div>
                //         </div>

                //     </div>
                // </div>

                div { class: if nav_hid == true {
                        "fixed left-0 top-0 w-full h-[100vh] bg-[#818a91]/70 z-50"
                    } else {
                        ""
                    },

                    div { class: if nav_hid == true {
                            "fixed left-0 top-0 w-[75%] sm:w-[45%] h-screen bg-white p-10 ease-in duration-200 shadow-l shadow-xl shadow-black"
                        } else {
                            "fixed left-[-200%] top-0 p-10 ease-in duration-200 shadow-l shadow-xl shadow-black"
                        },

                        div { class: "flex items-center justify-between w-full", img { src: "/images/128x128.png", width: 200, height: 200, alt: "" } }
                    }
                }
            }
        }
    }
}
