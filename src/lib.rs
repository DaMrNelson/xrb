pub mod models;
mod protocol;
mod xreaderwriter;

use std::collections::VecDeque;
use std::os::unix::net::UnixStream;
use std::io::prelude::*;
use std::io::{BufWriter, BufReader};

use std::thread;
use std::sync::mpsc;

use self::models::*;
use self::xreaderwriter::{XBufferedWriter, XBufferedReader, XReadHelper};

pub struct XClient {
    pub connected: bool,
    pub connect_info: ConnectInfo,
    buf_out: BufWriter<UnixStream>,
    resp_receiver: mpsc::Receiver<ServerResponse>, // Receive errors, replies, and events from the X Server
    resp_queue: VecDeque<ServerResponse>, // Used to store errors when the user wants to skip to a certain event or error
    sq_sender: mpsc::Sender<(u16, ServerReplyType)>, // Send sequence IDs to the reader thread so it can properly parse replies
    next_resource_id: u32,
    current_sequence: u16,
    buf_one_byte: Vec<u8>,
    buf_two_byte: Vec<u8>,
    buf_four_byte: Vec<u8>
}

impl XClient {
    /**
     * Connects to the given X server.
     * Blocks until the connection is complete.
     * Spawns a new 1:1 thread to constantly read input from the X Server, which prevents deadlocks.
     */
    pub fn connect(host: String) -> XClient {
        let stream = UnixStream::connect(host).unwrap();
        let (resp_sender, resp_receiver) = mpsc::channel();
        let (sq_sender, sq_receiver) = mpsc::channel();
        let mut client = XClient {
            connected: false,
            connect_info: ConnectInfo::empty(),
            buf_out: BufWriter::new(stream),
            resp_receiver: resp_receiver,
            resp_queue: VecDeque::with_capacity(15),
            sq_sender: sq_sender,
            next_resource_id: 0,
            current_sequence: 0,
            buf_one_byte: vec![0u8; 1],
            buf_two_byte: vec![0u8; 2],
            buf_four_byte: vec![0u8; 4]
        };
        client.setup(resp_sender, sq_receiver);
        client
    }

    /** Sends the connection parameters and returns if it connected or not. */
    fn setup(&mut self, resp_sender: mpsc::Sender<ServerResponse>, sq_receiver: mpsc::Receiver<(u16, ServerReplyType)>) {
        let mut reader = XReadHelper::new(self.buf_out.get_ref().try_clone().unwrap());

        // Send connection string
        {
            self.write_u8(protocol::CONNECT_LSB);
            self.write_pad(1);
            self.write_u16(protocol::CONNECT_MAJOR);
            self.write_u16(protocol::CONNECT_MINOR);
            self.write_u16(0);
            self.write_u16(0);
            //self.write_pad(4); // Pad empty string
            //self.write_pad(4); // Pad empty string
            self.write_pad(1); // Pad empty string
            self.write_pad(1); // Pad empty string

            self.write_sequence(ServerReplyType::None);
        }

        // Read response header
        {
            // Read the head
            reader.prep_read(8);
            self.connect_info.status_code = reader.read_u8();
            reader.read_pad(1);
            self.connect_info.protocol_major_version = reader.read_u16();
            self.connect_info.protocol_minor_version = reader.read_u16();
            self.connect_info.additional_data_len = reader.read_u16();

            // Check if the connection was a success
            // TODO: Parse body of failures
            match self.connect_info.status_code {
                protocol::CONNECT_SUCCESS => (),
                protocol::CONNECT_FAILED => panic!("Got CONNECT_FAILED"),
                protocol::CONNECT_AUTHENTICATE => panic!("Got CONNECT_AUTHENTICATE"),
                code => panic!("Got unexpected value {}", code),
            };

            // Parse success info
            println!("Server Protocol: {}.{}", self.connect_info.protocol_major_version, self.connect_info.protocol_minor_version);
            reader.prep_read((self.connect_info.additional_data_len * 4) as usize);
            self.connect_info.release_number = reader.read_u32();
            self.connect_info.resource_id_base = reader.read_u32();
            self.connect_info.resource_id_mask = reader.read_u32();
            self.connect_info.motion_buffer_size = reader.read_u32();
            let vendor_length = reader.read_u16();
            self.connect_info.max_request_length = reader.read_u16();
            self.connect_info.num_screens = reader.read_u8();
            self.connect_info.num_formats = reader.read_u8();
            self.connect_info.image_byte_order = match reader.read_u8() {
                0 => ByteOrder::LSBFirst,
                1 => ByteOrder::MSBFirst,
                order => panic!("Unknown image byte order {}", order),
            };
            self.connect_info.bitmap_format_bit_order = match reader.read_u8() {
                0 => BitOrder::LeastSignificant,
                1 => BitOrder::MostSignificant,
                order => panic!("Unknown bitmap format bit order {}", order)
            };
            self.connect_info.bitmap_format_scanline_unit = reader.read_u8();
            self.connect_info.bitmap_format_scanline_pad = reader.read_u8();
            self.connect_info.min_keycode = reader.read_char();
            self.connect_info.max_keycode = reader.read_char();
            reader.read_pad(4);

            self.connect_info.vendor = reader.read_str(vendor_length as usize);
            reader.read_pad((vendor_length as usize) % 4);
            println!("Server Vendor: {}", self.connect_info.vendor);

            // Formats (8 bytes each)
            for _ in 0..self.connect_info.num_formats {
                let mut format = Format::empty();
                format.depth = reader.read_u8();
                format.bits_per_pixel = reader.read_u8();
                format.scanline_pad = reader.read_u8();
                reader.read_pad(5);

                self.connect_info.formats.push(format);
            }

            // Read screens (x bytes, where x is a multiple of 4)
            for _ in 0..self.connect_info.num_screens {
                let mut screen = Screen::empty();
                screen.root = reader.read_u32();
                screen.default_colormap = reader.read_u32();
                screen.white_pixel = reader.read_u32();
                screen.black_pixel = reader.read_u32();
                screen.current_input_masks = reader.read_u32();
                screen.width_in_pixels = reader.read_u16();
                screen.height_in_pixels = reader.read_u16();
                screen.width_in_millimeters = reader.read_u16();
                screen.height_in_millimeters = reader.read_u16();
                screen.min_installed_maps = reader.read_u16();
                screen.max_installed_maps = reader.read_u16();
                screen.root_visual = reader.read_u32();
                screen.backing_stores = match reader.read_u8() {
                    0 => ScreenBackingStores::Never,
                    1 => ScreenBackingStores::WhenMapped,
                    2 => ScreenBackingStores::Always,
                    store => panic!("Unknown backing score {}", store)
                };
                screen.save_unders = reader.read_bool();
                screen.root_depth = reader.read_u8();
                screen.num_depths = reader.read_u8();

                // Read depths (x bytes, where x is a multiple of 4)
                for _ in 0..screen.num_depths {
                    let mut depth = Depth::empty();
                    depth.depth = reader.read_u8();
                    reader.read_pad(1);
                    depth.num_visuals = reader.read_u16();
                    reader.read_pad(4); // Unused
                    
                    // Read visuals (24 x num visuals bytes)
                    for _ in 0..depth.num_visuals {
                        let mut visual = Visual::empty();
                        visual.id = reader.read_u32();
                        visual.class = match reader.read_u8() {
                            0 => VisualType::StaticGray,
                            1 => VisualType::GrayScale,
                            2 => VisualType::StaticColor,
                            3 => VisualType::PseudoColor,
                            4 => VisualType::TrueColor,
                            5 => VisualType::DirectColor,
                            class => panic!("Unknown visual class {}", class)
                        };
                        visual.bits_per_rgb_value = reader.read_u8();
                        visual.colormap_entries = reader.read_u16();
                        visual.red_mask = reader.read_u32();
                        visual.green_mask = reader.read_u32();
                        visual.blue_mask = reader.read_u32();
                        reader.read_pad(4); // Unused

                        depth.visuals.push(visual);
                    }

                    screen.depths.push(depth);
                }

                self.connect_info.screens.push(screen);
            }
        }

        // Start event receiving thread
        {
            thread::spawn(move || { // Moves `resp_sender`, `sq_receiver`, and `reader`
                loop {
                    // Read header
                    reader.prep_read(32); // All are *at least* 32 bytes
                    let opcode = reader.read_u8();
                    let detail = reader.read_u8();
                    let sequence_number = reader.read_u16();

                    let response = match opcode {
                        protocol::REPLY_ERROR => {
                            sq_receiver.recv().unwrap(); // Throw out expected reply type
                            match reader.read_error(detail) {
                                Some(err) => ServerResponse::Error(err, sequence_number),
                                None => continue
                            }
                        },
                        protocol::REPLY_REPLY => ServerResponse::Reply({
                            let reply_length = reader.read_u32();
                            reader.prep_read_extend((reply_length * 4) as usize); // Add additional length
                            let (seq, method) = sq_receiver.recv().unwrap();

                            if seq == sequence_number {
                                match match method {
                                    ServerReplyType::GetWindowAttributes => reader.read_get_window_attributes_reply(detail),
                                    ServerReplyType::ListFontsWithInfo => reader.read_list_fonts_with_info_reply(detail),
                                    // TODO: More
                                    _ => panic!("Reply not implemented yet.")
                                } {
                                    Some(x) => x,
                                    None => continue
                                }
                            } else {
                                eprintln!("Unexpected reply sequence. Expected {}, got {}. Will not attempt to parse.", sequence_number, seq);
                                eprintln!("If this occurs again after restarting the server please submit an issue to: https://github.com/DaMrNelson/xrb");
                                reader.read_pad((32 - 4 + reply_length * 4) as usize);
                                continue
                            }
                        }, sequence_number),
                        other => {
                            ServerResponse::Event(
                                match match other { // MY EYES
                                    protocol::REPLY_KEY_PRESS => reader.read_key_press(detail),
                                    protocol::REPLY_KEY_RELEASE => reader.read_key_release(detail),
                                    protocol::REPLY_BUTTON_PRESS => reader.read_button_press(detail),
                                    protocol::REPLY_BUTTON_RELEASE => reader.read_button_release(detail),
                                    protocol::REPLY_MOTION_NOTIFY => reader.read_motion_notify(detail),
                                    protocol::REPLY_ENTER_NOTIFY => reader.read_enter_notify(detail),
                                    protocol::REPLY_LEAVE_NOTIFY => reader.read_leave_notify(detail),
                                    protocol::REPLY_FOCUS_IN => reader.read_focus_in(detail),
                                    protocol::REPLY_FOCUS_OUT => reader.read_focus_out(detail),
                                    protocol::REPLY_KEYMAP_NOTIFY => reader.read_keymap_notify(detail),
                                    protocol::REPLY_EXPOSE => reader.read_expose(),
                                    protocol::REPLY_GRAPHICS_EXPOSURE => reader.read_graphics_exposure(),
                                    protocol::REPLY_NO_EXPOSURE => reader.read_no_exposure(),
                                    protocol::REPLY_VISIBILITY_NOTIFY => reader.read_visibility_notify(),
                                    protocol::REPLY_CREATE_NOTIFY => reader.read_create_notify(),
                                    protocol::REPLY_DESTROY_NOTIFY => reader.read_destroy_notify(),
                                    protocol::REPLY_UNMAP_NOTIFY => reader.read_unmap_notify(),
                                    protocol::REPLY_MAP_NOTIFY => reader.read_map_notify(),
                                    protocol::REPLY_MAP_REQUEST => reader.read_map_request(),
                                    protocol::REPLY_REPARENT_NOTIFY => reader.read_reparent_notify(),
                                    protocol::REPLY_CONFIGURE_NOTIFY => reader.read_configure_notify(),
                                    protocol::REPLY_CONFIGURE_REQUEST => reader.read_configure_request(detail),
                                    protocol::REPLY_GRAVITY_NOTIFY => reader.read_gravity_notify(),
                                    protocol::REPLY_RESIZE_REQUEST => reader.read_resize_request(),
                                    protocol::REPLY_CIRCULATE_NOTIFY => reader.read_circulate_notify(),
                                    protocol::REPLY_CIRCULATE_REQUEST => reader.read_circulate_request(),
                                    protocol::REPLY_PROPERTY_NOTIFY => reader.read_property_notify(),
                                    protocol::REPLY_SELECTION_CLEAR => reader.read_selection_clear(),
                                    protocol::REPLY_SELECTION_REQUEST => reader.read_selection_request(),
                                    protocol::REPLY_SELECTION_NOTIFY => reader.read_selection_notify(),
                                    protocol::REPLY_COLORMAP_NOTIFY => reader.read_colormap_notify(),
                                    protocol::REPLY_CLIENT_MESSAGE => reader.read_client_message(detail),
                                    protocol::REPLY_MAPPING_NOTIFY => reader.read_mapping_notify(),
                                    _ => continue
                                } {
                                    Some(event) => event,
                                    None => continue
                                }
                            , sequence_number)
                        }
                    };

                    match resp_sender.send(response) {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to forward error, reply, or event to main thread: {:?}", e)
                    }
                }
            });
        }
    }

    /** Generates a new resource ID */
    pub fn new_resource_id(&mut self) -> u32 {
        // TODO: Thread lock
        // TODO: Allow re-use of released resources

        let id = self.next_resource_id;

        if id > self.connect_info.resource_id_mask {
            panic!("Out of resource IDs."); // Hopefully won't happen once re-using resource IDs is done
        }

        self.next_resource_id += 1;
        self.connect_info.resource_id_base | id
    }

    /**
     * Waits for the next available event, error, or reply. Blocks.
     * Use get_message if you do not want to block.
     */
    pub fn wait_for_message(&mut self) -> ServerResponse {
        loop {
            if self.resp_queue.is_empty() {
                match self.resp_receiver.recv() {
                    Ok(x) => return x,
                    Err(e) => eprintln!("Failed to get message from the receiver. Will try again. Error: {:?}", e)
                }
            } else {
                return self.resp_queue.pop_front().unwrap();
            }
        }
    }

    /**
     * Returns the next available event, error, or reply, or None if there is nothing yet. Does not block.
     * Use wait_for_message to block until a new message is received.
     */
    pub fn get_message(&mut self) -> Option<ServerResponse> {
        if self.resp_queue.is_empty() {
            match self.resp_receiver.try_recv() {
                Ok(x) => Some(x),
                Err(_) => None
            }
        } else {
            self.resp_queue.pop_front()
        }
    }

    /**
     * Waits for the next available error or reply with the given sequence number. Blocks.
     * Note: wait_for_message will also pick up responses. Only use this method if you want to ignore other requests until you get a response from a specific request.
     * Skipped messages will be saved and will be used in wait_for_message and get_message, in order.
     * 
     * Returns: ServerResponse::Error or ServerResponse::Reply.
     */
    pub fn wait_for_response(&mut self, seq: u16) -> ServerResponse {
        // Check if its in the current response queue
        if !self.resp_queue.is_empty() {
            let mut index = self.resp_queue.len();

            for (i, res) in self.resp_queue.iter().enumerate() {
                match res {
                    &ServerResponse::Error(_, eseq) => {
                        if eseq == seq {
                            index = i;
                            break;
                        }
                    },
                    &ServerResponse::Reply(_, eseq) => {
                        if eseq == seq {
                            index = i;
                            break;
                        }
                    },
                    _ => ()
                }
            }

            if index != self.resp_queue.len() {
                return self.resp_queue.remove(index).unwrap();
            }
        }

        // Start growing the response queue until we get that response
        let mut matched = false;

        loop {
            let mut val = None;

            match self.resp_receiver.recv() {
                Ok(res) => match res {
                        ServerResponse::Error(_, eseq) => {
                        println!("Trying to match. Expect {}, got {}", seq, eseq);
                        if eseq == seq {
                            matched = true;
                            val = Some(res);
                        }
                    },
                    ServerResponse::Reply(_, eseq) => {
                        println!("Trying to match. Expect {}, got {}", seq, eseq);
                        if eseq == seq {
                            matched = true;
                            val = Some(res);
                        }
                    },
                    _ => val = Some(res),
                },
                Err(e) => eprintln!("Failed to get message from the receiver. Will try again. Error: {:?}", e)
            };

            match val {
                Some(res) => {
                    if matched {
                        return res;
                    } else {
                    self.resp_queue.push_back(res);
                    }
                },
                None => ()
            }
        }
    }
}

// Endpoints
impl XClient { // This is actually a pretty nice feature for organization
    // COPY+PASTE TEMPLATE (not required to follow this format, it just makes it easier to write)
    /*
    /** Tells the X Server to [TODO] */
    pub fn (&mut self, ) -> u16 {
    pub fn (&mut self, ) {
        self.write_u8(protocol::OP_);

        self.write_sequence(ServerReplyType::None)
        self.write_request();
    }
    */

    /** Tells the X Server to create a window */
    pub fn create_window(&mut self, window: &Window) {
        // Should be 28 not including values and their mask
        self.write_u8(protocol::OP_CREATE_WINDOW);
        self.write_u8(window.depth);
        self.write_u16(8 + window.values.len() as u16); // data length
        self.write_u32(window.wid);
        self.write_u32(window.parent);
        self.write_i16(window.x);
        self.write_i16(window.y);
        self.write_u16(window.width);
        self.write_u16(window.height);
        self.write_u16(window.border_width);
        self.write_u16(match window.class {
            WindowInputType::CopyFromParent => 0,
            WindowInputType::InputOutput => 1,
            WindowInputType::InputOnly => 2
        });
        self.write_u32(window.visual_id);
        self.write_values(&window.values);

        self.write_request();
    }

    /** Tells the X Server to change a window's attributes */
    pub fn change_window_attributes(&mut self, wid: u32, values: &Vec<WindowValue>) {
        // Should be 28 not including values and their mask
        self.write_u8(protocol::OP_CHANGE_WINDOW_ATTRIBUTES);
        self.write_pad(1);
        self.write_u16(3 + values.len() as u16); // data length
        self.write_u32(wid);
        self.write_values(&values);

        self.write_request();
    }

    /** Tells the X Server to send us the window's attributes */
    pub fn get_window_attributes(&mut self, wid: u32) -> u16 {
        self.write_u8(protocol::OP_GET_WINDOW_ATTRIBUTES);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_sequence(ServerReplyType::GetWindowAttributes)
    }

    /** Tells the X Server to destroy a window */
    pub fn destroy_window(&mut self, wid: u32) {
        self.write_u8(protocol::OP_DESTROY_WINDOW);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_request();
    }

    /** Tells the X Server to destroy a window's subwidnows */
    pub fn destroy_subwindows(&mut self, wid: u32) {
        self.write_u8(protocol::OP_DESTROY_SUBWINDOWS);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_request();
    }

    /** Tells the X Server to change a window's save set */
    pub fn change_save_set(&mut self, wid: u32, mode: SaveSetMode) {
        self.write_u8(protocol::OP_CHANGE_SAVE_SET);
        self.write_u8(mode.val());
        self.write_u16(2);
        self.write_u32(wid);

        self.write_request();
    }

    /** Tells the X Server to reparent a window */
    pub fn reparent_window(&mut self, wid: u32, parent: u32, x: i16, y: i16) {
        self.write_u8(protocol::OP_REPARENT_WINDOW);
        self.write_pad(1);
        self.write_u16(4);
        self.write_u32(wid);
        self.write_u32(parent);
        self.write_i16(x);
        self.write_i16(y);

        self.write_request();
    }

    /** Tells the X Server to map a window (makes it visible I think) */
    pub fn map_window(&mut self, wid: u32) {
        self.write_u8(protocol::OP_MAP_WINDOW);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_request();
    }

    /** Tells the X Server to map a window's subwindows (makes them visible I think) */
    pub fn map_subwindows(&mut self, wid: u32) {
        self.write_u8(protocol::OP_MAP_SUBWINDOWS);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_request();
    }

    /** Tells the X Server to unmap a window (makes it invisible I think) */
    pub fn unmap_window(&mut self, wid: u32) {
        self.write_u8(protocol::OP_UNMAP_WINDOW);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_request();
    }

    /** Tells the X Server to unmap a window's subwindows (makes them invisible I think) */
    pub fn unmap_subwindows(&mut self, wid: u32) {
        self.write_u8(protocol::OP_UNMAP_SUBWINDOWS);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_request();
    }

    /** Tells the X Server to configure a window */
    pub fn configure_window(&mut self, wid: u32, values: Vec<WindowValue>) {
        self.write_u8(protocol::OP_CONFIGURE_WINDOW);
        self.write_pad(1);
        self.write_u16(3 + values.len() as u16);
        self.write_u32(wid);
        self.write_values(&values);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn circulate_window(&mut self, wid: u32, direction: CirculateDirection) {
        self.write_u8(protocol::OP_UNMAP_SUBWINDOWS);
        self.write_u8(direction.val());
        self.write_u16(2);
        self.write_u32(wid);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn get_geometry(&mut self, drawable: u32) -> u16 {
        self.write_u8(protocol::OP_GET_GEOMETRY);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(drawable);

        self.write_sequence(ServerReplyType::GetGeometry)
    }

    /** Tells the X Server to [TODO] */
    pub fn query_tree(&mut self, wid: u32) -> u16 {
        self.write_u8(protocol::OP_QUERY_TREE);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_sequence(ServerReplyType::QueryTree)
    }

    /** Tells the X Server to [TODO] */
    pub fn intern_atom(&mut self, name: &str, only_if_exists: bool) -> u16 {
        self.write_u8(protocol::OP_INTERN_ATOM);
        self.write_bool(only_if_exists);
        self.write_u16((2 + name.len() + name.len() % 4) as u16 / 4);
        self.write_u16(name.len() as u16);
        self.write_pad(2);
        self.write_str(name);
        self.write_pad_op(name.len() % 4);

        self.write_sequence(ServerReplyType::InternAtom)
    }

    /** Tells the X Server to [TODO] */
    pub fn get_atom_name(&mut self, atom: u32) -> u16 {
        self.write_u8(protocol::OP_GET_ATOM_NAME);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(4);

        self.write_sequence(ServerReplyType::GetAtomName)
    }

    /** Tells the X Server to [TODO] */
    pub fn change_property(&mut self, wid: u32, property: u32, ptype: u32, mode: PropertyChangeMode, data: &[u8]) {
        let len = data.len();
        let format =
            if len % 4 == 0 {
                32
            } else if len % 2 == 0 {
                16
            } else {
                8
            };
        self.write_u8(protocol::OP_CHANGE_PROPERTY);
        self.write_u8(mode.val());
        self.write_u16(6 + (data.len() / 4 + data.len() % 4) as u16);
        self.write_u32(wid);
        self.write_u32(property);
        self.write_u32(ptype);
        self.write_u8(format);
        self.write_pad(3);
        self.write_u32(match format {
            32 => len / 4,
            16 => len / 2,
            8 => len,
            _ => unreachable!()
        } as u32);
        self.write_raw(data);
        self.write_pad_op(data.len() % match format {
            32 => 4,
            16 => 2,
            8 => 1,
            _ => unreachable!()
        });

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn delete_property(&mut self, wid: u32, property: u32) {
        self.write_u8(protocol::OP_DELETE_PROPERTY);
        self.write_pad(1);
        self.write_u16(3);
        self.write_u32(wid);
        self.write_u32(property);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `ptype` = 0 = any property type
     */
    pub fn get_property(&mut self, wid: u32, property: u32, ptype: u32, delete: bool, long_offset: u32, long_length: u32) -> u16 {
        self.write_u8(protocol::OP_GET_PROPERTY);
        self.write_bool(delete);
        self.write_u16(6);
        self.write_u32(wid);
        self.write_u32(property);
        self.write_u32(ptype);

        self.write_sequence(ServerReplyType::GetProperty)
    }

    /** Tells the X Server to [TODO] */
    pub fn list_properties(&mut self, wid: u32) -> u16 {
        self.write_u8(protocol::OP_LIST_PROPERTIES);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_sequence(ServerReplyType::ListProperties)
    }

    /**
     * Tells the X Server to [TODO]
     * `owner` = 0 = none
     * `time` = 0 = current time
     */
    pub fn set_selection_owner(&mut self, owner: u32, selection: u32, time: u32) {
        self.write_u8(protocol::OP_SET_SELECTION_OWNER);
        self.write_pad(1);
        self.write_u16(4);
        self.write_u32(owner);
        self.write_u32(selection);
        self.write_u32(time);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn get_selection_owner(&mut self, selection: u32) -> u16 {
        self.write_u8(protocol::OP_GET_SELECTION_OWNER);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(selection);

        self.write_sequence(ServerReplyType::GetSelectionOwner)
    }

    /** Tells the X Server to [TODO]
     * `property` = 0 = none
     * `time` = 0 = current time
     */
    pub fn convert_selection(&mut self, requestor: u32, selection: u32, target: u32, property: u32, time: u32) {
        self.write_u8(protocol::OP_CONVERT_SELECTION);
        self.write_pad(1);
        self.write_u16(6);
        self.write_u32(requestor);
        self.write_u32(selection);
        self.write_u32(target);
        self.write_u32(property);
        self.write_u32(time);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO] 
     * `destination` = 0 = PointerWindow
     * `destination` = 1 = InputFocus
     */
    pub fn send_event(&mut self, event: ServerEvent, propagate: bool, destination: u32) {
        let seq = self.current_sequence;
        match event {
            ServerEvent::KeyPress { key_code, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_KEY_PRESS);
                self.write_u8(key_code);
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(root);
                self.write_u32(event);
                self.write_u32(child);
                self.write_i16(root_x);
                self.write_i16(root_y);
                self.write_i16(event_x);
                self.write_i16(event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(same_screen);
                self.write_pad(1);
            },
            ServerEvent::KeyRelease { key_code, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_KEY_RELEASE);
                self.write_u8(key_code);
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(root);
                self.write_u32(event);
                self.write_u32(child);
                self.write_i16(root_x);
                self.write_i16(root_y);
                self.write_i16(event_x);
                self.write_i16(event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(same_screen);
                self.write_pad(1);
            },
            ServerEvent::ButtonPress { button, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_BUTTON_PRESS);
                self.write_u8(button);
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(root);
                self.write_u32(event);
                self.write_u32(child);
                self.write_i16(root_x);
                self.write_i16(root_y);
                self.write_i16(event_x);
                self.write_i16(event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(same_screen);
                self.write_pad(1);
            },
            ServerEvent::ButtonRelease { button, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_BUTTON_RELEASE);
                self.write_u8(button);
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(root);
                self.write_u32(event);
                self.write_u32(child);
                self.write_i16(root_x);
                self.write_i16(root_y);
                self.write_i16(event_x);
                self.write_i16(event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(same_screen);
                self.write_pad(1);
            },
            ServerEvent::MotionNotify { detail, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_MOTION_NOTIFY);
                self.write_u8(detail.val());
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(root);
                self.write_u32(event);
                self.write_u32(child);
                self.write_i16(root_x);
                self.write_i16(root_y);
                self.write_i16(event_x);
                self.write_i16(event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(same_screen);
                self.write_pad(1);
            },
            ServerEvent::EnterNotify { detail, time, root, event, child, root_x, root_y, event_x, event_y, state, mode, same_screen, focus } => {
                self.write_u8(protocol::REPLY_ENTER_NOTIFY);
                self.write_u8(detail.val());
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(root);
                self.write_u32(event);
                self.write_u32(child);
                self.write_i16(root_x);
                self.write_i16(root_y);
                self.write_i16(event_x);
                self.write_i16(event_y);
                self.write_mask_u16(state.iter().map(|val| val.val()).collect());
                self.write_u8(mode.val());
                self.write_u8(
                    if same_screen && focus {
                        0x01 | 0x02
                    } else if same_screen {
                        0x02
                    } else if focus {
                        0x01
                    } else {
                        0xFC
                    }
                );
            },
            ServerEvent::LeaveNotify { detail, time, root, event, child, root_x, root_y, event_x, event_y, state, mode, same_screen, focus } => {
                self.write_u8(protocol::REPLY_LEAVE_NOTIFY);
                self.write_u8(detail.val());
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(root);
                self.write_u32(event);
                self.write_u32(child);
                self.write_i16(root_x);
                self.write_i16(root_y);
                self.write_i16(event_x);
                self.write_i16(event_y);
                self.write_mask_u16(state.iter().map(|val| val.val()).collect());
                self.write_u8(mode.val());
                self.write_u8(
                    if same_screen && focus {
                        0x01 | 0x02
                    } else if same_screen {
                        0x02
                    } else if focus {
                        0x01
                    } else {
                        0xFC
                    }
                );
            },
            ServerEvent::FocusIn { detail, event, mode } => {
                self.write_u8(protocol::REPLY_FOCUS_IN);
                self.write_u8(detail.val());
                self.write_u16(seq);
                self.write_u32(event);
                self.write_u8(mode.val());
                self.write_pad(23);
            },
            ServerEvent::FocusOut { detail, event, mode } => {
                self.write_u8(protocol::REPLY_FOCUS_OUT);
                self.write_u8(detail.val());
                self.write_u16(seq);
                self.write_u32(event);
                self.write_u8(mode.val());
                self.write_pad(23);
            },
            ServerEvent::KeymapNotify { } => {
                panic!("Not implemented yet"); // TODO: Do this
            },
            ServerEvent::Expose { window, x, y, width, height, count } => {
                self.write_u8(protocol::REPLY_EXPOSE);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(window);
                self.write_u16(x);
                self.write_u16(y);
                self.write_u16(width);
                self.write_u16(height);
                self.write_u16(count);
                self.write_pad(14);
            },
            ServerEvent::GraphicsExposure { drawable, x, y, width, height, minor_opcode, count, major_opcode } => {
                self.write_u8(protocol::REPLY_GRAPHICS_EXPOSURE);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(drawable);
                self.write_u16(x);
                self.write_u16(y);
                self.write_u16(width);
                self.write_u16(height);
                self.write_u16(minor_opcode);
                self.write_u16(count);
                self.write_u8(major_opcode);
                self.write_pad(11);
            },
            ServerEvent::NoExposure { drawable, minor_opcode, major_opcode } => {
                self.write_u8(protocol::REPLY_NO_EXPOSURE);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(drawable);
                self.write_u16(minor_opcode);
                self.write_u8(major_opcode);
                self.write_pad(21);
            },
            ServerEvent::VisibilityNotify { window, state } => {
                self.write_u8(protocol::REPLY_VISIBILITY_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(window);
                self.write_u8(state.val());
                self.write_pad(23);
            },
            ServerEvent::CreateNotify { parent, window, x, y, width, height, border_width, override_redirect } => {
                self.write_u8(protocol::REPLY_CREATE_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(parent);
                self.write_u32(window);
                self.write_i16(x);
                self.write_i16(y);
                self.write_u16(width);
                self.write_u16(height);
                self.write_u16(border_width);
                self.write_bool(override_redirect);
                self.write_pad(9);
            },
            ServerEvent::DestroyNotify { event, window } => {
                self.write_u8(protocol::REPLY_DESTROY_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(event);
                self.write_u32(window);
                self.write_pad(20);
            },
            ServerEvent::UnmapNotify { event, window, from_configure } => {
                self.write_u8(protocol::REPLY_UNMAP_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(event);
                self.write_u32(window);
                self.write_bool(from_configure);
                self.write_pad(19);
            },
            ServerEvent::MapNotify { event, window, override_redirect } => {
                self.write_u8(protocol::REPLY_MAP_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(event);
                self.write_u32(window);
                self.write_bool(override_redirect);
                self.write_pad(19);
            },
            ServerEvent::MapRequest { parent, window } => {
                self.write_u8(protocol::REPLY_MAP_REQUEST);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(parent);
                self.write_u32(window);
                self.write_pad(20);
            },
            ServerEvent::ReparentNotify { event, window, parent, x, y, override_redirect } => {
                self.write_u8(protocol::REPLY_REPARENT_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(event);
                self.write_u32(window);
                self.write_u32(parent);
                self.write_i16(x);
                self.write_i16(y);
                self.write_bool(override_redirect);
                self.write_pad(11);
            },
            ServerEvent::ConfigureNotify { event, window, above_sibling, x, y, width, height, border_width, override_redirect } => {
                self.write_u8(protocol::REPLY_CONFIGURE_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(event);
                self.write_u32(window);
                self.write_u32(above_sibling);
                self.write_i16(x);
                self.write_i16(y);
                self.write_u16(width);
                self.write_u16(height);
                self.write_u16(border_width);
                self.write_bool(override_redirect);
                self.write_pad(5);
            },
            ServerEvent::ConfigureRequest { stack_mode, parent, window, sibling, x, y, width, height, border_width, values } => {
                self.write_u8(protocol::REPLY_CONFIGURE_REQUEST);
                self.write_u8(stack_mode.val());
                self.write_u16(seq);
                self.write_u32(parent);
                self.write_u32(window);
                self.write_u32(sibling);
                self.write_i16(x);
                self.write_i16(y);
                self.write_u16(width);
                self.write_u16(height);
                self.write_u16(border_width);
                self.write_mask_u16(values.iter().map(|val| val.val()).collect());
                self.write_pad(4);
            },
            ServerEvent::GravityNotify { event, window, x, y } => {
                self.write_u8(protocol::REPLY_GRAVITY_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(event);
                self.write_u32(window);
                self.write_i16(x);
                self.write_i16(y);
                self.write_pad(16);
            },
            ServerEvent::ResizeRequest { window, width, height } => {
                self.write_u8(protocol::REPLY_RESIZE_REQUEST);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(window);
                self.write_u16(width);
                self.write_u16(height);
                self.write_pad(20);
            },
            ServerEvent::CirculateNotify { event, window, place } => {
                self.write_u8(protocol::REPLY_CIRCULATE_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(event);
                self.write_u32(window);
                self.write_pad(4); // TODO: Spec says this is type "window", but that it is "unused"???
                self.write_u8(place.val());
                self.write_pad(15);
            },
            ServerEvent::CirculateRequest { parent, window, place } => {
                self.write_u8(protocol::REPLY_CIRCULATE_REQUEST);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(parent);
                self.write_u32(window);
                self.write_pad(4);
                self.write_u8(place.val());
                self.write_pad(15);
            },
            ServerEvent::PropertyNotify { window, atom, time, state } => {
                self.write_u8(protocol::REPLY_PROPERTY_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(window);
                self.write_u32(atom);
                self.write_u32(time);
                self.write_u8(state.val());
                self.write_pad(15);
            },
            ServerEvent::SelectionClear { time, owner, selection } => {
                self.write_u8(protocol::REPLY_SELECTION_CLEAR);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(owner);
                self.write_u32(selection);
                self.write_pad(16);
            },
            ServerEvent::SelectionRequest { time, owner, requestor, selection, target, property } => {
                self.write_u8(protocol::REPLY_SELECTION_REQUEST);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(owner);
                self.write_u32(requestor);
                self.write_u32(selection);
                self.write_u32(target);
                self.write_u32(property);
                self.write_pad(4);
            },
            ServerEvent::SelectionNotify { time, requestor, selection, target, property } => {
                self.write_u8(protocol::REPLY_SELECTION_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(time);
                self.write_u32(requestor);
                self.write_u32(selection);
                self.write_u32(target);
                self.write_u32(property);
                self.write_pad(8);
            },
            ServerEvent::ColormapNotify { window, colormap, new, state } => {
                self.write_u8(protocol::REPLY_COLORMAP_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(window);
                self.write_u32(colormap);
                self.write_bool(new);
                self.write_u8(state.val());
                self.write_pad(18);
            },
            ServerEvent::ClientMessage { format, window, mtype, data } => {
                self.write_u8(protocol::REPLY_CLIENT_MESSAGE);
                self.write_u8(format);
                self.write_u16(seq);
                self.write_u32(window);
                self.write_u32(mtype);
                self.write_raw(&data);
            },
            ServerEvent::MappingNotify { request, first_keycode, count } => {
                self.write_u8(protocol::REPLY_MAPPING_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u8(request.val());
                self.write_u8(first_keycode as u8);
                self.write_u8(count);
                self.write_pad(25);
            }
        };
    }

    // TODO: Continue at GrabPointer
    // Don't forget about the template above (search "fn (")

    /** Lists all fonts with the given info */
    pub fn list_fonts_with_info(&mut self, max_names: u16, pattern: &str) -> u16 {
        self.write_u8(protocol::OP_LIST_FONTS_WITH_INFO);
        self.write_pad(1);
        self.write_u16(2 + (pattern.len() + pattern.len() % 4) as u16 / 4);
        self.write_u16(max_names);
        self.write_u16(pattern.len() as u16);
        self.write_str(pattern);
        self.write_pad_op(pattern.len() % 4);

        self.write_sequence(ServerReplyType::ListFontsWithInfo)
    }

    /** Tells the X Server to create a pixmap */
    pub fn create_pixmap(&mut self, pixmap: Pixmap) {
        self.write_u8(protocol::OP_CREATE_PIXMAP);
        self.write_u8(pixmap.depth);
        self.write_u16(4); // Request length
        self.write_u32(pixmap.pid);
        self.write_u32(pixmap.drawable);
        self.write_u16(pixmap.width);
        self.write_u16(pixmap.height);

        self.write_request();
    }

    /** Tells the X Server to create a graphics context */
    pub fn create_gc(&mut self, gc: GraphicsContext) {
        self.write_u8(protocol::OP_CREATE_GC);
        self.write_pad(1);
        self.write_u16(4 + gc.values.len() as u16);
        self.write_u32(gc.cid);
        self.write_u32(gc.drawable);
        self.write_values(&gc.values);

        self.write_request();
    }
}

impl XBufferedWriter for XClient {
    /** Flushes the buffer and writes a reply. */
    fn write_sequence(&mut self, rtype: ServerReplyType) -> u16 {
        match rtype {
            ServerReplyType::None => (),
            _ => self.sq_sender.send((self.current_sequence, rtype)).unwrap()
        };
        self.buf_out.flush().unwrap();
        let the_sequence = self.current_sequence;
        self.current_sequence += 1;
        the_sequence
    }

    /** Shortcut to write_sequence(ServerReplyType::None); */
    fn write_request(&mut self) {
        self.write_sequence(ServerReplyType::None);
    }

    /**
     * Writes raw data.
     */
    fn write_raw(&mut self, buf: &[u8]) {
        self.buf_out.write_all(buf);
    }

    /**
     * Writes 1 or more bytes (not guaranteed to be zero).
     */
    fn write_pad(&mut self, len: usize) {
        match len {
            0 => panic!("Cannot write 0 bytes"),
            1 => self.buf_out.write_all(&self.buf_one_byte),
            2 => self.buf_out.write_all(&self.buf_two_byte),
            4 => self.buf_out.write_all(&self.buf_four_byte),
            _ => self.buf_out.write_all(&vec![0u8; len])
        }.unwrap();
    }

    /**
     * Writes 0 or more bytes (not guaranteed to be zero).
     */
    fn write_pad_op(&mut self, len: usize) {
        if len != 0 {
            self.write_pad(len);
        }
    }

    /**
     * Writes a bool to the buffer.
     */
    fn write_bool(&mut self, input: bool) {
        self.buf_one_byte[0] = match input {
            true => 1,
            false => 0
        };
        self.buf_out.write_all(&self.buf_one_byte).unwrap();
    }

    /**
     * Writes a u8 to the buffer.
     */
    fn write_u8(&mut self, input: u8) {
        self.buf_one_byte[0] = input;
        self.buf_out.write_all(&self.buf_one_byte).unwrap();
    }

    /**
     * Writes a i16 to the buffer.
     * Expects little endian.
     */
    fn write_i16(&mut self, input: i16) {
        self.write_u16(input as u16);
    }

    /**
     * Writes a u16 to the buffer.
     * Expects little endian.
     */
    fn write_u16(&mut self, input: u16) {
        self.buf_two_byte[0] = input as u8;
        self.buf_two_byte[1] = (input >> 8) as u8;
        self.buf_out.write_all(&self.buf_two_byte).unwrap();
    }

    /**
     * Writes a i32 to the buffer.
     * Expects little endian.
     */
    fn write_i32(&mut self, input: i32) {
        self.write_u32(input as u32);
    }

    /**
     * Writes a u32 to the buffer.
     * Expects little endian.
     */
    fn write_u32(&mut self, input: u32) {
        self.buf_four_byte[0] = input as u8;
        self.buf_four_byte[1] = (input >> 8) as u8;
        self.buf_four_byte[2] = (input >> 16) as u8;
        self.buf_four_byte[3] = (input >> 24) as u8;
        self.buf_out.write_all(&self.buf_four_byte).unwrap();
    }

    /**
     * Writes a string to the buffer.
     * This does not write the length of the string or any padding required after it.
     */
    fn write_str(&mut self, input: &str) {
        match input.len() {
            0 => (),
            _ => self.buf_out.write_all(input.as_bytes()).unwrap()
        };
    }

    /**
     * Writes a mask based off some u16 values.
     */
    fn write_mask_u16(&mut self, input: Vec<u16>) {
        let mut mask = 0;

        for val in input.iter() {
            mask |= val;
        }
        
        self.write_u16(mask);
    }

    /**
     * Writes 4 bytes to the buffer, with the least significant being the bool.
     */
    fn write_val_bool(&mut self, input: bool) {
        match input {
            true => self.write_val(1u8 as u32),
            false => self.write_val(0u8 as u32)
        };
    }

    /**
     * Writes a 4-byte value to the buffer.
     * Expects little endian.
     */
    fn write_val_u8(&mut self, input: u8) {
        self.write_val(input as u32);
    }

    /**
     * Writes a 4-byte value to the buffer.
     * Expects little endian.
     */
    fn write_val_i16(&mut self, input: i16) {
        self.write_val_u16(input as u16);
    }

    /**
     * Writes a 4-byte value to the buffer.
     * Expects little endian.
     */
    fn write_val_u16(&mut self, input: u16) {
        self.write_val(input as u32);
    }

    /**
     * Writes a i32 to the buffer.
     * Expects little endian.
     */
    fn write_val_i32(&mut self, input: i32) {
        self.write_val(input as u32);
    }

    /**
     * Writes a u32 to the buffer.
     * Expects little endian.
     */
    fn write_val_u32(&mut self, input: u32) {
        self.write_val(input);
    }

    /**
     * Writes a u32 to the buffer.
     * Expects little endian.
     */
    fn write_val(&mut self, input: u32) {
        self.write_u32(input);
    }

    /**
     * Writes a bitmap and values to the buffer.
     */
    fn write_values<T: Value>(&mut self, values: &Vec<T>) {
        let mut value_mask: u32 = 0x0;
        let mut order: Vec<usize> = Vec::with_capacity(values.len());

        for (i, value) in values.iter().enumerate() {
            let ordered = value.get_mask();
            value_mask |= ordered;
            let mut pos = order.len();

            for (j, val) in order.iter().enumerate()  {
                if ordered < values[*val].get_mask() {
                    pos = j;
                    break;
                }
            }

            if pos < order.len() {
                order.insert(pos, i);
            } else {
                order.push(pos);
            }
        }

        self.write_u32(value_mask);

        for i in order.iter() {
            values[*i].write(self);
        }
    }
}
