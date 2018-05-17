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
        let mut client = XClient::connect(String::from("/tmp/.X11-unix/X9"));

        ///////////////////////////////////
        //// TESTING
        ///////////////////////////////////
        
        let bgsize = 500;
        let ibgsize = bgsize as i16;

        // Create a pixmap
        let pixmap = Pixmap {
            depth: client.info.screens[0].root_depth,
            pid: client.new_resource_id(),
            drawable: client.info.screens[0].root,
            width: bgsize,
            height: bgsize
        };
        client.create_pixmap(&pixmap);

        // Create GC (graphics context)
        let mut gc = GraphicsContext {
            gcid: client.new_resource_id(),
            drawable: client.info.screens[0].root,
            values: vec![
                GraphicsContextValue::Background(client.info.screens[0].black_pixel),
                GraphicsContextValue::Foreground(client.info.screens[0].black_pixel)
            ]
        };
        client.create_gc(&gc);
        let gcid = gc.gcid;

        // Draw backgorund and some arcs
        pixmap.fill_rect(&mut client, gcid, Rectangle { x: 0, y: 0, width: bgsize, height: bgsize });
        //client.poly_fill_rectangle(pixmap.pid, gcid, &vec![Rectangle { x: 0, y: 0, width: bgsize, height: bgsize }]);
        let white = client.info.screens[0].white_pixel;
        //client.change_gc(gc.gcid, &vec![GraphicsContextValue::Foreground(white)]);
        gc.set_fg(&mut client, &Color::from_num(0xFF0000));
        pixmap.draw_arcs(&mut client, gcid, &vec![
        //client.poly_arc(pixmap.pid, gcid, &vec![
            Arc { x: -ibgsize / 2, y: 0, width: bgsize, height: bgsize, angle1: 0, angle2: 360 * 64 },
            Arc { x: ibgsize / 2, y: 0, width: bgsize, height: bgsize, angle1: 0, angle2: 360 * 64 },
            Arc { x: 0, y: -ibgsize / 2, width: bgsize, height: bgsize, angle1: 0, angle2: 360 * 64 },
            Arc { x: 0, y: ibgsize / 2, width: bgsize, height: bgsize, angle1: 0, angle2: 360 * 64 }
        ]);

        // Create a window
        let mut window = Window {
            depth: client.info.screens[0].root_depth,
            wid: client.new_resource_id(),
            parent: client.info.screens[0].root,
            x: 20,
            y: 200,
            width: 500,
            height: 500,
            border_width: 0,
            class: WindowInputType::InputOutput,
            visual_id: 0, // CopyFromParent
            values: vec![
                WindowValue::BackgroundPixmap(pixmap.pid),
                WindowValue::EventMask(Event::ButtonRelease.val() | Event::StructureNotify.val()),
                WindowValue::Colormap(0x0)
            ]
        };
        client.create_window(&window);
        
        // Change the window a lil
        window.set(&mut client, WindowValue::EventMask(Event::ButtonRelease.val() | Event::ButtonPress.val() | Event::StructureNotify.val()));

        // Map the window (make it visible)
        client.map_window(window.wid);

        // Create a child window
        let child = Window {
            depth: client.info.screens[0].root_depth,
            wid: client.new_resource_id(),
            parent: window.wid,
            x: 20,
            y: 20,
            width: 20,
            height: 20,
            border_width: 0,
            class: WindowInputType::CopyFromParent,
            visual_id: 0, // CopyFromParent
            values: vec![
                WindowValue::BackgroundPixel(0x00FFFF),
                WindowValue::Colormap(0)
            ]
        };
        client.create_window(&child);
        client.map_window(child.wid);

        // Test replies
        let seq = client.get_window_attributes(window.wid);
        println!("Expecting response with sequence {}", seq);
        match client.wait_for_response(seq) {
            ServerResponse::Error(error, sequence_number) => {
                println!("Got error response {}: {:?}", sequence_number, error);
            },
            ServerResponse::Reply(reply, sequence_number) => {
                println!("Got reply response {}: {:?}", sequence_number, reply);
            },
            _ => unreachable!()
        };
        //client.list_fonts_with_info("", 5);

        // Main event loop
        loop {
            match client.wait_for_message() {
                ServerResponse::Error(error, sequence_number) => {
                    println!("Got error {}: {:?}", sequence_number, error);
                },
                ServerResponse::Reply(reply, sequence_number) => {
                    println!("Got reply {}: {:?}", sequence_number, reply);
                },
                ServerResponse::Event(event, sequence_number) => {
                    println!("Got event {}: {:?}", sequence_number, event);
                }
            }
        }
    }
}