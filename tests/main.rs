extern crate xrb;

// Tests
#[cfg(test)]
mod tests {
    use xrb::XClient;
    use xrb::models::*;

    #[test]
    fn main_test() {
        // Connect
        //let mut client = XClient::new(String::from("/tmp/.X11-unix/X1"));
        let mut client = XClient::new(String::from("/tmp/.X11-unix/X9"));
        client.connect();

        ///////////////////////////////////
        //// TESTING
        ///////////////////////////////////

        // Create a pixmap
        let pixmap = Pixmap {
            depth: client.connect_info.screens[0].root_depth,
            pid: client.new_resource_id(),
            drawable: client.connect_info.screens[0].root,
            width: 20,
            height: 20
        };
        let pid = pixmap.pid;

        client.create_pixmap(pixmap);

        // Create GC (graphics context)
        let gc = GraphicsContext {
            cid: client.new_resource_id(),
            drawable: client.connect_info.screens[0].root,
            values: vec![
                GraphicsContextValue::Background(client.connect_info.screens[0].black_pixel),
                GraphicsContextValue::Foreground(client.connect_info.screens[0].white_pixel)
            ]
        };

        client.create_gc(gc);

        // Create a window
        let mut window = Window {
            depth: client.connect_info.screens[0].root_depth,
            wid: client.new_resource_id(),
            parent: client.connect_info.screens[0].root,
            x: 20,
            y: 200,
            width: 500,
            height: 500,
            border_width: 0,
            class: WindowInputType::InputOutput,
            visual_id: 0, // CopyFromParent
            values: vec![
                WindowValue::BackgroundPixmap(pid),
                WindowValue::EventMask(Event::ButtonRelease.val() | Event::StructureNotify.val()),
                WindowValue::Colormap(0x0)
            ]
        };
        client.create_window(&window);
        
        // Change the window a lil
        window.set_attr(&mut client, WindowValue::EventMask(Event::ButtonRelease.val() | Event::ButtonPress.val() | Event::StructureNotify.val()));

        // Map the window (make it visible)
        client.map_window(window.wid);

        // Main event loop
        loop {
            match client.wait_for_message() {
                ServerResponse::Error(error) => {
                    println!("Got error: {:?}", error);
                },
                ServerResponse::Event(event) => {
                    println!("Got event: {:?}", event);
                }
            }
        }
    }
}