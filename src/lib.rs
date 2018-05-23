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
    pub info: ConnectInfo,
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
            info: ConnectInfo::empty(),
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
            self.info.status_code = reader.read_u8();
            reader.read_pad(1);
            self.info.protocol_major_version = reader.read_u16();
            self.info.protocol_minor_version = reader.read_u16();
            self.info.additional_data_len = reader.read_u16();

            // Check if the connection was a success
            // TODO: Parse body of failures
            match self.info.status_code {
                protocol::CONNECT_SUCCESS => (),
                protocol::CONNECT_FAILED => panic!("Got CONNECT_FAILED"),
                protocol::CONNECT_AUTHENTICATE => panic!("Got CONNECT_AUTHENTICATE"),
                code => panic!("Got unexpected value {}", code),
            };

            // Parse success info
            println!("Server Protocol: {}.{}", self.info.protocol_major_version, self.info.protocol_minor_version);
            reader.prep_read((self.info.additional_data_len * 4) as usize);
            self.info.release_number = reader.read_u32();
            self.info.resource_id_base = reader.read_u32();
            self.info.resource_id_mask = reader.read_u32();
            self.info.motion_buffer_size = reader.read_u32();
            let vendor_length = reader.read_u16();
            self.info.max_request_length = reader.read_u16();
            self.info.num_screens = reader.read_u8();
            self.info.num_formats = reader.read_u8();
            self.info.image_byte_order = match reader.read_u8() {
                0 => ByteOrder::LSBFirst,
                1 => ByteOrder::MSBFirst,
                order => panic!("Unknown image byte order {}", order),
            };
            self.info.bitmap_format_bit_order = match reader.read_u8() {
                0 => BitOrder::LeastSignificant,
                1 => BitOrder::MostSignificant,
                order => panic!("Unknown bitmap format bit order {}", order)
            };
            self.info.bitmap_format_scanline_unit = reader.read_u8();
            self.info.bitmap_format_scanline_pad = reader.read_u8();
            self.info.min_keycode = reader.read_char();
            self.info.max_keycode = reader.read_char();
            reader.read_pad(4);

            self.info.vendor = reader.read_str(vendor_length as usize);
            reader.read_pad((vendor_length as usize) % 4);
            println!("Server Vendor: {}", self.info.vendor);

            // Formats (8 bytes each)
            for _ in 0..self.info.num_formats {
                let mut format = Format::empty();
                format.depth = reader.read_u8();
                format.bits_per_pixel = reader.read_u8();
                format.scanline_pad = reader.read_u8();
                reader.read_pad(5);

                self.info.formats.push(format);
            }

            // Read screens (x bytes, where x is a multiple of 4)
            for _ in 0..self.info.num_screens {
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

                self.info.screens.push(screen);
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
                                    ServerReplyType::GetWindowAttributes => reader.read_get_window_attributes_reply(detail),
                                    ServerReplyType::GetGeometry => reader.read_get_geometry_reply(detail),
                                    ServerReplyType::QueryTree => reader.read_query_tree_reply(),
                                    ServerReplyType::InternAtom => reader.read_intern_atom_reply(),
                                    ServerReplyType::GetAtomName => reader.read_get_atom_name_reply(),
                                    ServerReplyType::GetProperty => reader.read_get_property_reply(detail),
                                    ServerReplyType::ListProperties => reader.read_list_properties_reply(),
                                    ServerReplyType::GetSelectionOwner => reader.read_get_selection_owner_reply(),
                                    ServerReplyType::GrabPointer => reader.read_grab_pointer_reply(detail),
                                    ServerReplyType::GrabKeyboard => reader.read_grab_keyboard_reply(detail),
                                    ServerReplyType::QueryPointer => reader.read_query_pointer_reply(detail),
                                    ServerReplyType::GetMotionEvents => reader.read_get_motion_events_reply(),
                                    ServerReplyType::TranslateCoordinates => reader.read_translate_coordinates_reply(detail),
                                    ServerReplyType::GetInputFocus => reader.read_get_input_focus_reply(detail),
                                    ServerReplyType::QueryKeymap => reader.read_query_keymap_reply(),
                                    ServerReplyType::QueryFont => reader.read_query_font_reply(),
                                    ServerReplyType::QueryTextExtents => reader.read_query_text_extents_reply(detail),
                                    ServerReplyType::ListFonts => reader.read_list_fonts_reply(),
                                    ServerReplyType::ListFontsWithInfo => reader.read_list_fonts_with_info_reply(detail), // Note: One request will generate multiple replies here. The info specifies how to determine this
                                    ServerReplyType::GetFontPath => reader.read_get_font_path_reply(),
                                    ServerReplyType::GetImage => reader.read_get_image_reply(detail),
                                    ServerReplyType::ListInstalledColormaps => reader.read_list_installed_colormaps_reply(),
                                    ServerReplyType::AllocColor => reader.read_alloc_color_reply(),
                                    ServerReplyType::AllocNamedColor => reader.read_alloc_named_color_reply(),
                                    ServerReplyType::AllocColorCells => reader.read_alloc_color_cells_reply(),
                                    ServerReplyType::AllocColorPlanes => reader.read_alloc_color_planes_reply(),
                                    ServerReplyType::QueryColors => reader.read_query_colors_reply(),
                                    ServerReplyType::LookupColor => reader.read_lookup_color_reply(),
                                    ServerReplyType::QueryBestSize => reader.read_query_best_size_reply(),
                                    ServerReplyType::QueryExtension => reader.read_query_extension_reply(),
                                    ServerReplyType::ListExtensions => reader.read_list_extensions_reply(detail),
                                    ServerReplyType::GetKeyboardMapping => reader.read_get_keyboard_mapping_reply(detail),
                                    ServerReplyType::GetKeyboardControl => reader.read_get_keyboard_control_reply(detail),
                                    ServerReplyType::GetPointerControl => reader.read_get_pointer_control_reply(),
                                    ServerReplyType::GetScreenSaver => reader.read_get_screen_saver_reply(),
                                    ServerReplyType::ListHosts => reader.read_list_hosts_reply(detail),
                                    ServerReplyType::SetPointerMapping => reader.read_set_pointer_mapping_reply(detail),
                                    ServerReplyType::GetPointerMapping => reader.read_get_pointer_mapping_reply(detail),
                                    ServerReplyType::SetModifierMapping => reader.read_set_modifier_mapping_reply(detail),
                                    ServerReplyType::GetModifierMapping => reader.read_get_modifier_mapping_reply(detail),
                                    ServerReplyType::None => panic!("Reply type should not be none.")
                                } {
                                    Some(x) => x,
                                    None => continue
                                }
                            } else {
                                // This is expected, as errors do not consume from the type queue
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

        if id > self.info.resource_id_mask {
            panic!("Out of resource IDs."); // Hopefully won't happen once re-using resource IDs is done
        }

        self.next_resource_id += 1;
        self.info.resource_id_base | id
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
                Ok(res) => {
                    match res {
                        ServerResponse::Error(_, eseq) => {
                            if eseq == seq {
                                matched = true;
                            }
                        },
                        ServerResponse::Reply(_, eseq) => {
                            if eseq == seq {
                                matched = true;
                            }
                        },
                        _ => ()
                    };
                    val = Some(res);
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

// Spec Endpoints
impl XClient { // This is actually a pretty nice feature for organization
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
        self.write_values(&window.values, 32);

        self.write_request();
    }

    /** Tells the X Server to change a window's attributes */
    pub fn change_window_attributes(&mut self, wid: u32, values: &Vec<WindowValue>) {
        // Should be 28 not including values and their mask
        self.write_u8(protocol::OP_CHANGE_WINDOW_ATTRIBUTES);
        self.write_pad(1);
        self.write_u16(3 + values.len() as u16); // data length
        self.write_u32(wid);
        self.write_values(&values, 32);

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
    pub fn change_save_set(&mut self, wid: u32, mode: &SaveSetMode) {
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
    pub fn configure_window(&mut self, wid: u32, values: &Vec<WindowConfigureValue>) {
        self.write_u8(protocol::OP_CONFIGURE_WINDOW);
        self.write_pad(1);
        self.write_u16(3 + values.len() as u16);
        self.write_u32(wid);
        self.write_values(&values, 16);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn circulate_window(&mut self, wid: u32, direction: &CirculateDirection) {
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
        let pad = self.write_dynamic_len(2, name.len());
        self.write_u16(name.len() as u16);
        self.write_pad(2);
        self.write_str(name);
        self.write_pad_op(pad);

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
    pub fn change_property(&mut self, wid: u32, property: u32, ptype: u32, mode: &PropertyChangeMode, data: &[u8]) {
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
        let pad = self.write_dynamic_len(6, data.len());
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
        self.write_u32(long_offset);
        self.write_u32(long_length);

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
    pub fn send_event(&mut self, event: &ServerEvent, propagate: bool, destination: u32, events: &Vec<Event>) {
        self.write_u8(protocol::OP_SEND_EVENT);
        self.write_bool(propagate);
        self.write_u16(11);
        self.write_u32(destination);
        self.write_mask_u32(events.iter().map(|val| val.val()).collect());

        // Write body
        let seq = self.current_sequence;
        match event {
            ServerEvent::KeyPress { key_code, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_KEY_PRESS);
                self.write_u8(*key_code);
                self.write_u16(seq);
                self.write_u32(*time);
                self.write_u32(*root);
                self.write_u32(*event);
                self.write_u32(*child);
                self.write_i16(*root_x);
                self.write_i16(*root_y);
                self.write_i16(*event_x);
                self.write_i16(*event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(*same_screen);
                self.write_pad(1);
            },
            ServerEvent::KeyRelease { key_code, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_KEY_RELEASE);
                self.write_u8(*key_code);
                self.write_u16(seq);
                self.write_u32(*time);
                self.write_u32(*root);
                self.write_u32(*event);
                self.write_u32(*child);
                self.write_i16(*root_x);
                self.write_i16(*root_y);
                self.write_i16(*event_x);
                self.write_i16(*event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(*same_screen);
                self.write_pad(1);
            },
            ServerEvent::ButtonPress { button, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_BUTTON_PRESS);
                self.write_u8(*button);
                self.write_u16(seq);
                self.write_u32(*time);
                self.write_u32(*root);
                self.write_u32(*event);
                self.write_u32(*child);
                self.write_i16(*root_x);
                self.write_i16(*root_y);
                self.write_i16(*event_x);
                self.write_i16(*event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(*same_screen);
                self.write_pad(1);
            },
            ServerEvent::ButtonRelease { button, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_BUTTON_RELEASE);
                self.write_u8(*button);
                self.write_u16(seq);
                self.write_u32(*time);
                self.write_u32(*root);
                self.write_u32(*event);
                self.write_u32(*child);
                self.write_i16(*root_x);
                self.write_i16(*root_y);
                self.write_i16(*event_x);
                self.write_i16(*event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(*same_screen);
                self.write_pad(1);
            },
            ServerEvent::MotionNotify { detail, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen } => {
                self.write_u8(protocol::REPLY_MOTION_NOTIFY);
                self.write_u8(detail.val());
                self.write_u16(seq);
                self.write_u32(*time);
                self.write_u32(*root);
                self.write_u32(*event);
                self.write_u32(*child);
                self.write_i16(*root_x);
                self.write_i16(*root_y);
                self.write_i16(*event_x);
                self.write_i16(*event_y);
                self.write_u16({
                    let mut mask = 0;
                    for val in state.iter() {
                        mask |= val.val();
                    }
                    mask
                });
                self.write_bool(*same_screen);
                self.write_pad(1);
            },
            ServerEvent::EnterNotify { detail, time, root, event, child, root_x, root_y, event_x, event_y, state, mode, same_screen, focus } => {
                self.write_u8(protocol::REPLY_ENTER_NOTIFY);
                self.write_u8(detail.val());
                self.write_u16(seq);
                self.write_u32(*time);
                self.write_u32(*root);
                self.write_u32(*event);
                self.write_u32(*child);
                self.write_i16(*root_x);
                self.write_i16(*root_y);
                self.write_i16(*event_x);
                self.write_i16(*event_y);
                self.write_mask_u16(state.iter().map(|val| val.val()).collect());
                self.write_u8(mode.val());
                self.write_u8(
                    if *same_screen && *focus {
                        0x01 | 0x02
                    } else if *same_screen {
                        0x02
                    } else if *focus {
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
                self.write_u32(*time);
                self.write_u32(*root);
                self.write_u32(*event);
                self.write_u32(*child);
                self.write_i16(*root_x);
                self.write_i16(*root_y);
                self.write_i16(*event_x);
                self.write_i16(*event_y);
                self.write_mask_u16(state.iter().map(|val| val.val()).collect());
                self.write_u8(mode.val());
                self.write_u8(
                    if *same_screen && *focus {
                        0x01 | 0x02
                    } else if *same_screen {
                        0x02
                    } else if *focus {
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
                self.write_u32(*event);
                self.write_u8(mode.val());
                self.write_pad(23);
            },
            ServerEvent::FocusOut { detail, event, mode } => {
                self.write_u8(protocol::REPLY_FOCUS_OUT);
                self.write_u8(detail.val());
                self.write_u16(seq);
                self.write_u32(*event);
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
                self.write_u32(*window);
                self.write_u16(*x);
                self.write_u16(*y);
                self.write_u16(*width);
                self.write_u16(*height);
                self.write_u16(*count);
                self.write_pad(14);
            },
            ServerEvent::GraphicsExposure { drawable, x, y, width, height, minor_opcode, count, major_opcode } => {
                self.write_u8(protocol::REPLY_GRAPHICS_EXPOSURE);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*drawable);
                self.write_u16(*x);
                self.write_u16(*y);
                self.write_u16(*width);
                self.write_u16(*height);
                self.write_u16(*minor_opcode);
                self.write_u16(*count);
                self.write_u8(*major_opcode);
                self.write_pad(11);
            },
            ServerEvent::NoExposure { drawable, minor_opcode, major_opcode } => {
                self.write_u8(protocol::REPLY_NO_EXPOSURE);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*drawable);
                self.write_u16(*minor_opcode);
                self.write_u8(*major_opcode);
                self.write_pad(21);
            },
            ServerEvent::VisibilityNotify { window, state } => {
                self.write_u8(protocol::REPLY_VISIBILITY_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*window);
                self.write_u8(state.val());
                self.write_pad(23);
            },
            ServerEvent::CreateNotify { parent, window, x, y, width, height, border_width, override_redirect } => {
                self.write_u8(protocol::REPLY_CREATE_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*parent);
                self.write_u32(*window);
                self.write_i16(*x);
                self.write_i16(*y);
                self.write_u16(*width);
                self.write_u16(*height);
                self.write_u16(*border_width);
                self.write_bool(*override_redirect);
                self.write_pad(9);
            },
            ServerEvent::DestroyNotify { event, window } => {
                self.write_u8(protocol::REPLY_DESTROY_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*event);
                self.write_u32(*window);
                self.write_pad(20);
            },
            ServerEvent::UnmapNotify { event, window, from_configure } => {
                self.write_u8(protocol::REPLY_UNMAP_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*event);
                self.write_u32(*window);
                self.write_bool(*from_configure);
                self.write_pad(19);
            },
            ServerEvent::MapNotify { event, window, override_redirect } => {
                self.write_u8(protocol::REPLY_MAP_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*event);
                self.write_u32(*window);
                self.write_bool(*override_redirect);
                self.write_pad(19);
            },
            ServerEvent::MapRequest { parent, window } => {
                self.write_u8(protocol::REPLY_MAP_REQUEST);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*parent);
                self.write_u32(*window);
                self.write_pad(20);
            },
            ServerEvent::ReparentNotify { event, window, parent, x, y, override_redirect } => {
                self.write_u8(protocol::REPLY_REPARENT_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*event);
                self.write_u32(*window);
                self.write_u32(*parent);
                self.write_i16(*x);
                self.write_i16(*y);
                self.write_bool(*override_redirect);
                self.write_pad(11);
            },
            ServerEvent::ConfigureNotify { event, window, above_sibling, x, y, width, height, border_width, override_redirect } => {
                self.write_u8(protocol::REPLY_CONFIGURE_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*event);
                self.write_u32(*window);
                self.write_u32(*above_sibling);
                self.write_i16(*x);
                self.write_i16(*y);
                self.write_u16(*width);
                self.write_u16(*height);
                self.write_u16(*border_width);
                self.write_bool(*override_redirect);
                self.write_pad(5);
            },
            ServerEvent::ConfigureRequest { stack_mode, parent, window, sibling, x, y, width, height, border_width, values } => {
                self.write_u8(protocol::REPLY_CONFIGURE_REQUEST);
                self.write_u8(stack_mode.val());
                self.write_u16(seq);
                self.write_u32(*parent);
                self.write_u32(*window);
                self.write_u32(*sibling);
                self.write_i16(*x);
                self.write_i16(*y);
                self.write_u16(*width);
                self.write_u16(*height);
                self.write_u16(*border_width);
                self.write_mask_u16(values.iter().map(|val| val.val()).collect());
                self.write_pad(4);
            },
            ServerEvent::GravityNotify { event, window, x, y } => {
                self.write_u8(protocol::REPLY_GRAVITY_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*event);
                self.write_u32(*window);
                self.write_i16(*x);
                self.write_i16(*y);
                self.write_pad(16);
            },
            ServerEvent::ResizeRequest { window, width, height } => {
                self.write_u8(protocol::REPLY_RESIZE_REQUEST);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*window);
                self.write_u16(*width);
                self.write_u16(*height);
                self.write_pad(20);
            },
            ServerEvent::CirculateNotify { event, window, place } => {
                self.write_u8(protocol::REPLY_CIRCULATE_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*event);
                self.write_u32(*window);
                self.write_pad(4); // TODO: Spec says this is type "window", but that it is "unused"???
                self.write_u8(place.val());
                self.write_pad(15);
            },
            ServerEvent::CirculateRequest { parent, window, place } => {
                self.write_u8(protocol::REPLY_CIRCULATE_REQUEST);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*parent);
                self.write_u32(*window);
                self.write_pad(4);
                self.write_u8(place.val());
                self.write_pad(15);
            },
            ServerEvent::PropertyNotify { window, atom, time, state } => {
                self.write_u8(protocol::REPLY_PROPERTY_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*window);
                self.write_u32(*atom);
                self.write_u32(*time);
                self.write_u8(state.val());
                self.write_pad(15);
            },
            ServerEvent::SelectionClear { time, owner, selection } => {
                self.write_u8(protocol::REPLY_SELECTION_CLEAR);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*time);
                self.write_u32(*owner);
                self.write_u32(*selection);
                self.write_pad(16);
            },
            ServerEvent::SelectionRequest { time, owner, requestor, selection, target, property } => {
                self.write_u8(protocol::REPLY_SELECTION_REQUEST);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*time);
                self.write_u32(*owner);
                self.write_u32(*requestor);
                self.write_u32(*selection);
                self.write_u32(*target);
                self.write_u32(*property);
                self.write_pad(4);
            },
            ServerEvent::SelectionNotify { time, requestor, selection, target, property } => {
                self.write_u8(protocol::REPLY_SELECTION_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*time);
                self.write_u32(*requestor);
                self.write_u32(*selection);
                self.write_u32(*target);
                self.write_u32(*property);
                self.write_pad(8);
            },
            ServerEvent::ColormapNotify { window, colormap, new, state } => {
                self.write_u8(protocol::REPLY_COLORMAP_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u32(*window);
                self.write_u32(*colormap);
                self.write_bool(*new);
                self.write_u8(state.val());
                self.write_pad(18);
            },
            ServerEvent::ClientMessage { format, window, mtype, data } => {
                self.write_u8(protocol::REPLY_CLIENT_MESSAGE);
                self.write_u8(*format);
                self.write_u16(seq);
                self.write_u32(*window);
                self.write_u32(*mtype);
                self.write_raw(data);
            },
            ServerEvent::MappingNotify { request, first_keycode, count } => {
                self.write_u8(protocol::REPLY_MAPPING_NOTIFY);
                self.write_pad(1);
                self.write_u16(seq);
                self.write_u8(request.val());
                self.write_char(*first_keycode);
                self.write_u8(*count);
                self.write_pad(25);
            }
        };
    }

    /**
     * Tells the X Server to [TODO]
     * `confine_to` = 0 = none
     * `cursor` = 0 = none
     * `time` = 0 = current time
     */
    pub fn grab_pointer(&mut self, grab_window: u32, confine_to: u32, cursor: u32, events: &Vec<PointerEvent>, pointer_mode: &PointerMode, keyboard_mode: &KeyboardMode, owner_events: bool, time: u32) -> u16 {
        self.write_u8(protocol::OP_GRAB_POINTER);
        self.write_bool(owner_events);
        self.write_u16(6);
        self.write_u32(grab_window);
        self.write_mask_u16(events.iter().map(|val| val.val()).collect());
        self.write_u8(pointer_mode.val());
        self.write_u8(keyboard_mode.val());
        self.write_u32(confine_to);
        self.write_u32(cursor);
        self.write_u32(time);

        self.write_sequence(ServerReplyType::GrabPointer)
    }

    /**
     * Tells the X Server to [TODO] 
     * `time` = 0 = current time
     */
    pub fn ungrab_pointer(&mut self, time: u32) {
        self.write_u8(protocol::OP_UNGRAB_POINTER);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(time);

        self.write_request();
    }
    
    /**
     * Tells the X Server to [TODO]
     * `confine-to` = 0 = none
     * `cursor` = 0 = none
     * `button` = 0 = any button
     * `modifiers` = 0x8000 = any modifier
     */
    pub fn grab_button(&mut self, button: u8, grab_window: u32, confine_to: u32, cursor: u32, events: &Vec<PointerEvent>, pointer_mode: &PointerMode, keyboard_mode: &KeyboardMode, modifiers: &Vec<Key>, owner_events: bool) {
        self.write_u8(protocol::OP_GRAB_BUTTON);
        self.write_bool(owner_events);
        self.write_u16(6);
        self.write_u32(grab_window);
        self.write_mask_u16(events.iter().map(|val| val.val()).collect());
        self.write_u8(pointer_mode.val());
        self.write_u8(keyboard_mode.val());
        self.write_u32(confine_to);
        self.write_u32(cursor);
        self.write_u8(button);
        self.write_pad(1);
        self.write_mask_u16(modifiers.iter().map(|val| val.val()).collect());

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO] 
     * `button` = 0 = any button
     * `modifiers` = 0x8000 = any modifier
     */
    pub fn ungrab_button(&mut self, button: u8, grab_window: u32, modifiers: &Vec<Key>) {
        self.write_u8(protocol::OP_UNGRAB_BUTTON);
        self.write_u8(button);
        self.write_u16(3);
        self.write_u32(grab_window);
        self.write_mask_u16(modifiers.iter().map(|val| val.val()).collect());
        self.write_pad(2);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `cursor` = 0 = none
     * `time` = 0 = current time
     */
    pub fn change_active_pointer_grab(&mut self, cursor: u32, time: u32, events: &Vec<PointerEvent>) {
        self.write_u8(protocol::OP_CHANGE_ACTIVE_POINTER_GRAB);
        self.write_pad(1);
        self.write_u16(4);
        self.write_u32(cursor);
        self.write_u32(time);
        self.write_mask_u16(events.iter().map(|val| val.val()).collect());
        self.write_pad(2);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `time` = 0 = current time
     */
    pub fn grab_keyboard(&mut self, grab_window: u32, pointer_mode: &PointerMode, keyboard_mode: &KeyboardMode, owner_events: bool, time: u32) -> u16 {
        self.write_u8(protocol::OP_GRAB_KEYBOARD);
        self.write_bool(owner_events);
        self.write_u16(4);
        self.write_u32(grab_window);
        self.write_u32(time);
        self.write_u8(pointer_mode.val());
        self.write_u8(keyboard_mode.val());
        self.write_pad(2);

        self.write_sequence(ServerReplyType::GrabKeyboard)
    }

    /**
     * Tells the X Server to [TODO]
     * `time` = 0 = current time
     */
    pub fn ungrab_keyboard(&mut self, time: u32) {
        self.write_u8(protocol::OP_UNGRAB_KEYBOARD);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(time);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `modifiers` = 0x8000 = any modifier
     * `key` = 0 = any key
     */
    pub fn grab_key(&mut self, key: char, grab_window: u32, pointer_mode: &PointerMode, keyboard_mode: &KeyboardMode, modifiers: &Vec<Key>, owner_events: bool) {
        self.write_u8(protocol::OP_GRAB_KEY);
        self.write_bool(owner_events);
        self.write_u16(4);
        self.write_u32(grab_window);
        self.write_mask_u16(modifiers.iter().map(|val| val.val()).collect());
        self.write_char(key);
        self.write_u8(pointer_mode.val());
        self.write_u8(keyboard_mode.val());
        self.write_pad(3);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `key` = 0 = any key
     * `modifiers` = 0x8000 = any modifier
     */
    pub fn ungrab_key(&mut self, key: char, grab_window: u32, modifiers: &Vec<Key>) {
        self.write_u8(protocol::OP_UNGRAB_KEY);
        self.write_char(key);
        self.write_u16(3);
        self.write_u32(grab_window);
        self.write_mask_u16(modifiers.iter().map(|val| val.val()).collect());
        self.write_pad(2);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn grab_server(&mut self) {
        self.write_u8(protocol::OP_GRAB_SERVER);
        self.write_pad(1);
        self.write_u16(1);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn ungrab_server(&mut self) {
        self.write_u8(protocol::OP_UNGRAB_SERVER);
        self.write_pad(1);
        self.write_u16(1);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn query_pointer(&mut self, wid: u32) -> u16 {
        self.write_u8(protocol::OP_QUERY_POINTER);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_sequence(ServerReplyType::QueryPointer)
    }

    /**
     * Tells the X Server to [TODO]
     * `start` = 0 = current time
     * `stop` = 0 = current time
     */
    pub fn get_motion_events(&mut self, wid: u32, start: u32, stop: u32) -> u16 {
        self.write_u8(protocol::OP_GET_MOTION_EVENTS);
        self.write_pad(1);
        self.write_u16(4);
        self.write_u32(wid);
        self.write_u32(start);
        self.write_u32(stop);

        self.write_sequence(ServerReplyType::QueryPointer)
    }

    /** Tells the X Server to [TODO] */
    pub fn translate_coordinates(&mut self, src_window: u32, dst_window: u32, src_x: i16, src_y: i16) -> u16 {
        self.write_u8(protocol::OP_TRANSLATE_COORDINATES);
        self.write_pad(1);
        self.write_u16(4);
        self.write_u32(src_window);
        self.write_u32(dst_window);
        self.write_i16(src_x);
        self.write_i16(src_y);

        self.write_sequence(ServerReplyType::QueryPointer)
    }

    /** Tells the X Server to [TODO] */
    pub fn warp_pointer(&mut self, src_window: u32, dst_window: u32, src_x: i16, src_y: i16, src_width: u16, src_height: u16, dst_x: i16, dst_y: i16) {
        self.write_u8(protocol::OP_WARP_POINTER);
        self.write_pad(1);
        self.write_u16(6);
        self.write_u32(src_window);
        self.write_u32(dst_window);
        self.write_i16(src_x);
        self.write_i16(src_y);
        self.write_u16(src_width);
        self.write_u16(src_height);
        self.write_i16(dst_x);
        self.write_i16(dst_y);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `time` = 0 = current time
     */
    pub fn set_input_focus(&mut self, focus: u32, revert_to: &InputFocusRevert, time: u32) {
        self.write_u8(protocol::OP_SET_INPUT_FOCUS);
        self.write_u8(revert_to.val());
        self.write_u16(3);
        self.write_u32(focus);
        self.write_u32(time);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn get_input_focus(&mut self) -> u16 {
        self.write_u8(protocol::OP_GET_INPUT_FOCUS);
        self.write_pad(1);
        self.write_u16(1);

        self.write_sequence(ServerReplyType::GetInputFocus)
    }

    /** Tells the X Server to [TODO] */
    pub fn query_keymap(&mut self) -> u16 {
        self.write_u8(protocol::OP_QUERY_KEYMAP);
        self.write_pad(1);
        self.write_u16(1);

        self.write_sequence(ServerReplyType::QueryKeymap)
    }

    /** Tells the X Server to [TODO] */
    pub fn open_font(&mut self, fid: u32, name: &str) {
        self.write_u8(protocol::OP_OPEN_FONT);
        self.write_pad(1);
        let pad = self.write_dynamic_len(3, name.len());
        self.write_u32(fid);
        self.write_u16(name.len() as u16);
        self.write_pad(2);
        self.write_str(name);
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn close_font(&mut self, fid: u32) {
        self.write_u8(protocol::OP_CLOSE_FONT);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(fid);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn query_font(&mut self, fid: u32) -> u16 {
        self.write_u8(protocol::OP_QUERY_FONT);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(fid);

        self.write_sequence(ServerReplyType::QueryFont)
    }

    /** Tells the X Server to [TODO] */
    pub fn query_text_extents(&mut self, fid: u32, text: &str) -> u16 {
        self.write_u8(protocol::OP_QUERY_TEXT_EXTENTS);
        self.write_bool(text.len() % 2 == 1);
        let pad = self.write_dynamic_len(2, text.len());
        self.write_u32(fid);
        self.write_str(text);
        self.write_pad_op(pad);

        self.write_sequence(ServerReplyType::QueryTextExtents)
    }

    /** Tells the X Server to [TODO] */
    pub fn list_fonts(&mut self, pattern: &str, max_names: u16) -> u16 {
        self.write_u8(protocol::OP_LIST_FONTS);
        self.write_pad(1);
        let pad = self.write_dynamic_len(2, pattern.len());
        self.write_u16(max_names);
        self.write_u16(pattern.len() as u16);
        self.write_str(pattern);
        self.write_pad_op(pad);

        self.write_sequence(ServerReplyType::ListFonts)
    }

    /** Lists all fonts with the given info */
    pub fn list_fonts_with_info(&mut self, pattern: &str, max_names: u16) -> u16 {
        self.write_u8(protocol::OP_LIST_FONTS_WITH_INFO);
        self.write_pad(1);
        let pad = self.write_dynamic_len(2, pattern.len());
        self.write_u16(max_names);
        self.write_u16(pattern.len() as u16);
        self.write_str(pattern);
        self.write_pad_op(pad);

        self.write_sequence(ServerReplyType::ListFontsWithInfo)
    }

    /** Tells the X Server to [TODO] */
    pub fn set_font_path(&mut self) {
        self.write_u8(protocol::OP_SET_FONT_PATH);
        panic!("Not implemented yet"); // TODO
        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn get_font_path(&mut self) -> u16 {
        self.write_u8(protocol::OP_GET_FONT_PATH);
        self.write_pad(1);
        self.write_u16(1);

        self.write_sequence(ServerReplyType::GetFontPath)
    }

    /** Tells the X Server to create a pixmap */
    pub fn create_pixmap(&mut self, pixmap: &Pixmap) {
        self.write_u8(protocol::OP_CREATE_PIXMAP);
        self.write_u8(pixmap.depth);
        self.write_u16(4); // Request length
        self.write_u32(pixmap.pid);
        self.write_u32(pixmap.drawable);
        self.write_u16(pixmap.width);
        self.write_u16(pixmap.height);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn free_pixmap(&mut self, pixmap: u32) {
        self.write_u8(protocol::OP_FREE_PIXMAP);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(pixmap);

        self.write_request();
    }

    /** Tells the X Server to create a graphics context */
    pub fn create_gc(&mut self, gc: &GraphicsContext) {
        self.write_u8(protocol::OP_CREATE_GC);
        self.write_pad(1);
        self.write_u16(4 + gc.values.len() as u16);
        self.write_u32(gc.gcid);
        self.write_u32(gc.drawable);
        self.write_values(&gc.values, 32);

        self.write_request();
    }

    /** Tells the X Server to create a graphics context */
    pub fn change_gc(&mut self, gcid: u32, values: &Vec<GraphicsContextValue>) {
        self.write_u8(protocol::OP_CHANGE_GC);
        self.write_pad(1);
        self.write_u16(3 + values.len() as u16);
        self.write_u32(gcid);
        self.write_values(&values, 32);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn copy_gc(&mut self, src_gc: u32, dst_gc: u32, values_to_copy: &Vec<GraphicsContextMask>) {
        self.write_u8(protocol::OP_COPY_GC);
        self.write_pad(1);
        self.write_u16(4);
        self.write_u32(src_gc);
        self.write_u32(dst_gc);
        self.write_mask_u32(values_to_copy.iter().map(|val| val.val()).collect());

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn set_dashes(&mut self, gcid: u32, offset: u16, dashes: &Vec<u8>) {
        self.write_u8(protocol::OP_SET_DASHES);
        self.write_pad(1);
        let pad = self.write_dynamic_len(3, dashes.len());
        self.write_u32(gcid);
        self.write_u16(offset);
        self.write_u16(dashes.len() as u16);
        for dash in dashes {
            self.write_u8(*dash);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn set_clip_rectangles(&mut self, gcid: u32, rectangles: &Vec<Rectangle>, clip_x_origin: i16, clip_y_origin: i16, ordering: &RectangleOrdering) {
        self.write_u8(protocol::OP_SET_CLIP_RECTANGLES);
        self.write_u8(ordering.val());
        let pad = self.write_dynamic_len(3, rectangles.len() * 8);
        self.write_u32(gcid);
        self.write_i16(clip_x_origin);
        self.write_i16(clip_y_origin);
        for rect in rectangles {
            rect.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn free_gc(&mut self, gcid: u32) {
        self.write_u8(protocol::OP_FREE_GC);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(gcid);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn clear_area(&mut self, wid: u32, x: i16, y: i16, width: u16, height: u16, exposures: bool) {
        self.write_u8(protocol::OP_CLEAR_AREA);
        self.write_bool(exposures);
        self.write_u16(4);
        self.write_u32(wid);
        self.write_i16(x);
        self.write_i16(y);
        self.write_u16(width);
        self.write_u16(height);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn copy_plane(&mut self, src: u32, dst: u32, gcid: u32, src_x: i16, src_y: i16, dst_x: i16, dst_y: i16, width: u16, height: u16, bit_plane: u32) {
        self.write_u8(protocol::OP_COPY_PLANE);
        self.write_pad(1);
        self.write_u16(8);
        self.write_u32(src);
        self.write_u32(dst);
        self.write_u32(gcid);
        self.write_i16(src_x);
        self.write_i16(src_y);
        self.write_i16(dst_x);
        self.write_i16(dst_y);
        self.write_u16(width);
        self.write_u16(height);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn poly_point(&mut self, drawable: u32, gcid: u32, points: &Vec<Point>, mode: &CoordinateMode) {
        self.write_u8(protocol::OP_POLY_POINT);
        self.write_u8(mode.val());
        let pad = self.write_dynamic_len(3, points.len() * 4);
        self.write_u32(drawable);
        self.write_u32(gcid);
        for point in points {
            point.write(self);
        }
        self.write_pad(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn poly_line(&mut self, drawable: u32, gcid: u32, points: &Vec<Point>, mode: &CoordinateMode) {
        self.write_u8(protocol::OP_POLY_LINE);
        self.write_u8(mode.val());
        let pad = self.write_dynamic_len(3, points.len() * 4);
        self.write_u32(drawable);
        self.write_u32(gcid);
        for point in points {
            point.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn poly_segment(&mut self, drawable: u32, gcid: u32, segments: &Vec<Segment>) {
        self.write_u8(protocol::OP_POLY_SEGMENT);
        self.write_pad(1);
        let pad = self.write_dynamic_len(3, segments.len() * 8);
        self.write_u32(drawable);
        self.write_u32(gcid);
        for segment in segments {
            segment.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn poly_rectangle(&mut self, drawable: u32, gcid: u32, rectangles: &Vec<Rectangle>) {
        self.write_u8(protocol::OP_POLY_RECTANGLE);
        self.write_pad(1);
        let pad = self.write_dynamic_len(3, rectangles.len() * 8);
        self.write_u32(drawable);
        self.write_u32(gcid);
        for rect in rectangles {
            rect.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn poly_arc(&mut self, drawable: u32, gcid: u32, arcs: &Vec<Arc>) {
        self.write_u8(protocol::OP_POLY_ARC);
        self.write_pad(1);
        let pad = self.write_dynamic_len(3, arcs.len() * 12);
        self.write_u32(drawable);
        self.write_u32(gcid);
        for arc in arcs {
            arc.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn fill_poly(&mut self, drawable: u32, gcid: u32, points: &Vec<Point>, shape: &PolyShape, mode: &CoordinateMode) {
        self.write_u8(protocol::OP_FILL_POLY);
        self.write_pad(1);
        let pad = self.write_dynamic_len(4, points.len() * 4);
        self.write_u32(drawable);
        self.write_u32(gcid);
        self.write_u8(shape.val());
        self.write_u8(mode.val());
        self.write_pad(2);
        for point in points {
            point.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn poly_fill_rectangle(&mut self, drawable: u32, gcid: u32, rectangles: &Vec<Rectangle>) {
        self.write_u8(protocol::OP_POLY_FILL_RECTANGLE);
        self.write_pad(1);
        let pad = self.write_dynamic_len(3, rectangles.len() * 8);
        self.write_u32(drawable);
        self.write_u32(gcid);
        for rect in rectangles {
            rect.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn poly_fill_arc(&mut self, drawable: u32, gcid: u32, arcs: &Vec<Arc>) {
        self.write_u8(protocol::OP_POLY_FILL_ARC);
        self.write_pad(1);
        let pad = self.write_dynamic_len(3, arcs.len() * 12);
        self.write_u32(drawable);
        self.write_u32(gcid);
        for arc in arcs {
            arc.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn put_image(&mut self, drawable: u32, gcid: u32, data: &Vec<u8>, width: u16, height: u16, x: i16, y: i16, left_pad: u8, depth: u8, format: &ImageFormat) {
        self.write_u8(protocol::OP_PUT_IMAGE);
        self.write_u8(format.val());
        let pad = self.write_dynamic_len(6, data.len());
        self.write_u32(drawable);
        self.write_u32(gcid);
        self.write_u16(width);
        self.write_u16(height);
        self.write_i16(x);
        self.write_i16(y);
        self.write_u8(left_pad);
        self.write_u8(depth);
        self.write_pad(2);
        self.write_raw(&data);
        self.write_pad_op(pad);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `format` may only be ImageFormat::XYPixmap or ImageFormat::ZPixmap
     */
    pub fn get_image(&mut self, drawable: u32, x: i16, y: i16, width: u16, height: u16, plane_mask: u32, format: &ImageFormat) -> u16 {
        self.write_u8(protocol::OP_GET_IMAGE);
        self.write_u8(format.val());
        self.write_u16(5);
        self.write_u32(drawable);
        self.write_i16(x);
        self.write_i16(y);
        self.write_u16(width);
        self.write_u16(height);
        self.write_u32(plane_mask);

        self.write_sequence(ServerReplyType::GetImage)
    }

    /**
     * Tells the X Server to [TODO]
     * `texts` is TextItem8Text or TextItem8Font
     * A TextItem8Text entry in `texts` must be 254 or less characters
     */
    pub fn poly_text8<T: TextItem8>(&mut self, drawable: u32, gcid: u32, x: i16, y: i16, texts: &Vec<T>) {
        let mut len = 0;
        for text in texts {
            len += text.len();
        }

        self.write_u8(protocol::OP_POLY_TEXT8);
        self.write_pad(1);
        let pad = self.write_dynamic_len(4, len);
        self.write_u32(drawable);
        self.write_u32(gcid);
        self.write_i16(x);
        self.write_i16(y);
        for text in texts {
            text.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `texts` is TextItem16Text or TextItem16Font
     * A TextItem16Text entry in `texts` must be 254 or less characters
     */
    pub fn poly_text16<T: TextItem16>(&mut self, drawable: u32, gcid: u32, x: i16, y: i16, texts: &Vec<T>) {
        let mut len = 0;
        for text in texts {
            len += text.len();
        }

        self.write_u8(protocol::OP_POLY_TEXT16);
        self.write_pad(1);
        let pad = self.write_dynamic_len(4, len);
        self.write_u32(drawable);
        self.write_u32(gcid);
        self.write_i16(x);
        self.write_i16(y);
        for text in texts {
            text.write(self);
        }
        self.write_pad_op(pad);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO] 
     * `text` must be 255 or less characters
     */
    pub fn image_text8(&mut self, drawable: u32, gcid: u32, text: &str, x: i16, y: i16) {
        self.write_u8(protocol::OP_IMAGE_TEXT8);
        self.write_u8(text.len() as u8);
        let pad = self.write_dynamic_len(4, text.len());
        self.write_u32(drawable);
        self.write_u32(gcid);
        self.write_i16(x);
        self.write_i16(y);
        self.write_str(text);
        self.write_pad_op(pad);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `text` must have 255 or less elements
     */
    pub fn image_text16(&mut self, drawable: u32, gcid: u32, text: &Vec<u16>, x: i16, y: i16) {
        self.write_u8(protocol::OP_IMAGE_TEXT16);
        self.write_u8(text.len() as u8);
        let pad = self.write_dynamic_len(4, text.len() * 2);
        self.write_u32(drawable);
        self.write_u32(gcid);
        self.write_i16(x);
        self.write_i16(y);
        self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn create_colormap(&mut self, cmid: u32, wid: u32, vid: u32, mode: &AllocMode) {
        self.write_u8(protocol::OP_CREATE_COLORMAP);
        self.write_u8(mode.val());
        self.write_u16(4);
        self.write_u32(cmid);
        self.write_u32(wid);
        self.write_u32(vid);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn free_colormap(&mut self, cmid: u32) {
        self.write_u8(protocol::OP_FREE_COLORMAP);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(cmid);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn copy_colormap_and_free(&mut self, src_cmid: u32, dst_cmid: u32) {
        self.write_u8(protocol::OP_COPY_COLORMAP_AND_FREE);
        self.write_pad(1);
        self.write_u16(3);
        self.write_u32(dst_cmid);
        self.write_u32(src_cmid);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn install_colormap(&mut self, cmid: u32) {
        self.write_u8(protocol::OP_INSTALL_COLORMAP);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(cmid);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn uninstall_colormap(&mut self, cmid: u32) {
        self.write_u8(protocol::OP_UNINSTALL_COLORMAP);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(cmid);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn list_installed_colormaps(&mut self, wid: u32) -> u16 {
        self.write_u8(protocol::OP_LIST_INSTALLED_COLORMAPS);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_sequence(ServerReplyType::ListInstalledColormaps)
    }

    /** Tells the X Server to [TODO] */
    pub fn alloc_color(&mut self, cmid: u32, red: u16, green: u16, blue: u16) -> u16 {
        self.write_u8(protocol::OP_ALLOC_COLOR);
        self.write_pad(1);
		self.write_u16(4);
		self.write_u32(cmid);
		self.write_u16(red);
		self.write_u16(green);
		self.write_u16(blue);
		self.write_pad(2);

        self.write_sequence(ServerReplyType::AllocColor)
    }

    /** Tells the X Server to [TODO] */
    pub fn alloc_named_color(&mut self, cmid: u32, name: &str) -> u16 {
        self.write_u8(protocol::OP_ALLOC_NAMED_COLOR);
        self.write_pad(1);
		let pad = self.write_dynamic_len(3, name.len());
		self.write_u32(cmid);
		self.write_u16(name.len() as u16);
		self.write_pad(2);
        self.write_str(name);
		self.write_pad_op(pad);

        self.write_sequence(ServerReplyType::AllocNamedColor)
    }

    /** Tells the X Server to [TODO] */
    pub fn alloc_color_cells(&mut self, cmid: u32, colors: u16, planes: u16, contiguous: bool) -> u16 {
        self.write_u8(protocol::OP_ALLOC_COLOR_CELLS);
        self.write_bool(contiguous);
		self.write_u16(3);
		self.write_u32(cmid);
		self.write_u16(colors);
		self.write_u16(planes);

        self.write_sequence(ServerReplyType::AllocColorCells)
    }

    /** Tells the X Server to [TODO] */
    pub fn alloc_color_planes(&mut self, cmid: u32, colors: u16, reds: u16, greens: u16, blues: u16, contiguous: bool) -> u16 {
        self.write_u8(protocol::OP_ALLOC_COLOR_PLANES);
        self.write_bool(contiguous);
		self.write_u16(4);
		self.write_u32(cmid);
		self.write_u16(colors);
		self.write_u16(reds);
		self.write_u16(greens);
		self.write_u16(blues);

        self.write_sequence(ServerReplyType::AllocColorPlanes)
    }

    /** Tells the X Server to [TODO] */
    pub fn free_colors(&mut self, cmid: u32, plane_mask: u32, pixels: &Vec<u32>) {
        self.write_u8(protocol::OP_FREE_COLORS);
        self.write_pad(1);
		let pad = self.write_dynamic_len(3, pixels.len() * 4);
		self.write_u32(cmid);
		self.write_u32(plane_mask);
        for pixel in pixels {
            self.write_u32(*pixel);
        }
		self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn store_colors(&mut self, cmid: u32, items: &Vec<ColorItem>) {
        self.write_u8(protocol::OP_STORE_COLORS);
        self.write_pad(1);
		let pad = self.write_dynamic_len(2, items.len() * 12);
		self.write_u32(cmid);
        for item in items {
            item.write(self);
        }
		self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn store_named_color(&mut self, cmid: u32, name: &str, pixel: u32, do_red: bool, do_green: bool, do_blue: bool) {
        let mut mask = 0x00;
        if do_red {
            mask |= 0x01;
        }
        if do_green {
            mask |= 0x02;
        }
        if do_blue {
            mask |= 0x04;
        }

        self.write_u8(protocol::OP_STORE_NAMED_COLOR);
        self.write_u8(mask);
		let pad = self.write_dynamic_len(4, name.len());
		self.write_u32(cmid);
		self.write_u32(pixel);
		self.write_u16(name.len() as u16);
		self.write_pad(2);
        self.write_str(name);
		self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn query_colors(&mut self, cmid: u32, pixels: &Vec<u32>) -> u16 {
        self.write_u8(protocol::OP_QUERY_COLORS);
        self.write_pad(1);
		let pad = self.write_dynamic_len(2, pixels.len() * 4);
		self.write_u32(cmid);
        for pixel in pixels {
            self.write_u32(*pixel);
        }
		self.write_pad_op(pad);

        self.write_sequence(ServerReplyType::QueryColors)
    }

    /** Tells the X Server to [TODO] */
    pub fn lookup_color(&mut self, cmid: u32, name: &str) -> u16 {
        self.write_u8(protocol::OP_LOOKUP_COLOR);
        self.write_pad(1);
		let pad = self.write_dynamic_len(3, name.len());
		self.write_u32(cmid);
		self.write_u16(name.len() as u16);
		self.write_pad(2);
        self.write_str(name);
		self.write_pad_op(pad);

        self.write_sequence(ServerReplyType::LookupColor)
    }

    /**
     * Tells the X Server to [TODO]
     * `mask` = 0 = none
     */
    pub fn create_cursor(&mut self, cid: u32, source: u32, mask: u32, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16, x: u16, y: u16) {
        self.write_u8(protocol::OP_CREATE_CURSOR);
        self.write_pad(1);
		self.write_u16(8);
		self.write_u32(cid);
		self.write_u32(source);
		self.write_u32(mask);
		self.write_u16(fore_red);
		self.write_u16(fore_green);
		self.write_u16(fore_blue);
		self.write_u16(back_red);
		self.write_u16(back_green);
		self.write_u16(back_blue);
		self.write_u16(x);
		self.write_u16(y);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `mask_font` = 0 = none
     */
    pub fn create_glyph_cursor(&mut self, cid: u32, source_font: u32, mask_font: u32, source_char: u16, mask_char: u16, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16) {
        self.write_u8(protocol::OP_CREATE_GLYPH_CURSOR);
        self.write_pad(1);
		self.write_u16(8);
		self.write_u32(cid);
		self.write_u32(source_font);
		self.write_u32(mask_font);
		self.write_u16(source_char);
		self.write_u16(mask_char);
		self.write_u16(fore_red);
		self.write_u16(fore_green);
		self.write_u16(fore_blue);
		self.write_u16(back_red);
		self.write_u16(back_green);
		self.write_u16(back_blue);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn free_cursor(&mut self, cid: u32) {
        self.write_u8(protocol::OP_FREE_CURSOR);
        self.write_pad(1);
		self.write_u16(2);
		self.write_u32(cid);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn recolor_cursor(&mut self, cid: u32, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16) {
        self.write_u8(protocol::OP_RECOLOR_CURSOR);
        self.write_pad(1);
		self.write_u16(5);
		self.write_u32(cid);
		self.write_u16(fore_red);
		self.write_u16(fore_green);
		self.write_u16(fore_blue);
		self.write_u16(back_red);
		self.write_u16(back_green);
		self.write_u16(back_blue);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn query_best_size(&mut self, drawable: u32, class: &SizeClass, width: u16, height: u16) -> u16 {
        self.write_u8(protocol::OP_QUERY_BEST_SIZE);
        self.write_u8(class.val());
		self.write_u16(3);
		self.write_u32(drawable);
		self.write_u16(width);
		self.write_u16(height);

        self.write_sequence(ServerReplyType::QueryBestSize)
    }

    /** Tells the X Server to [TODO] */
    pub fn query_extension(&mut self, name: &str) -> u16 {
        self.write_u8(protocol::OP_QUERY_EXTENSION);
        self.write_pad(1);
		let pad = self.write_dynamic_len(2, name.len());
		self.write_u16(name.len() as u16);
		self.write_pad(2);
        self.write_str(name);
		self.write_pad_op(pad);

        self.write_sequence(ServerReplyType::QueryExtension)
    }

    /** Tells the X Server to [TODO] */
    pub fn list_extensions(&mut self) -> u16 {
        self.write_u8(protocol::OP_LIST_EXTENSIONS);
        self.write_pad(1);
		self.write_u16(1);

        self.write_sequence(ServerReplyType::ListExtensions)
    }

    /** Tells the X Server to [TODO] */
    pub fn change_keyboard_mapping(&mut self, first: char, keysyms: &Vec<u32>) {
        self.write_u8(protocol::OP_CHANGE_KEYBOARD_MAPPING);
        panic!("Not implemented yet");

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn get_keyboard_mapping(&mut self, first: char, count: u8) -> u16 {
        self.write_u8(protocol::OP_GET_KEYBOARD_MAPPING);
        self.write_pad(1);
		self.write_u16(2);
		self.write_char(first);
		self.write_u8(count);
		self.write_pad(2);

        self.write_sequence(ServerReplyType::GetKeyboardMapping)
    }

    /** Tells the X Server to [TODO] */
    pub fn change_keyboard_control(&mut self, values: &Vec<KeyboardControlValue>) {
        self.write_u8(protocol::OP_CHANGE_KEYBOARD_CONTROL);
        self.write_pad(1);
		let pad = self.write_dynamic_len(2, values.len() * 4);
		self.write_values(&values, 32);
		self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn get_keyboard_control(&mut self) -> u16 {
        self.write_u8(protocol::OP_GET_KEYBOARD_CONTROL);
        self.write_pad(1);
		self.write_u16(1);

        self.write_sequence(ServerReplyType::GetKeyboardControl)
    }

    /** Tells the X Server to [TODO] */
    pub fn bell(&mut self, percent: i8) {
        self.write_u8(protocol::OP_BELL);
        self.write_i8(percent);
		self.write_u16(1);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn change_pointer_control(&mut self, acceleration_numerator: i16, acceleration_denominator: i16, threshold: i16, do_acceleration: bool, do_threshold: bool) {
        self.write_u8(protocol::OP_CHANGE_POINTER_CONTROL);
        self.write_pad(1);
		self.write_u16(3);
		self.write_i16(acceleration_numerator);
		self.write_i16(acceleration_denominator);
		self.write_i16(threshold);
		self.write_bool(do_acceleration);
		self.write_bool(do_threshold);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn get_pointer_control(&mut self) -> u16 {
        self.write_u8(protocol::OP_GET_POINTER_CONTROL);
        self.write_pad(1);
		self.write_u16(1);

        self.write_sequence(ServerReplyType::GetPointerControl)
    }

    /** Tells the X Server to [TODO] */
    pub fn set_screen_saver(&mut self, timeout: i16, interval: i16, prefer_blanking: &YesNoDefault, allow_exposures: &YesNoDefault) {
        self.write_u8(protocol::OP_SET_SCREEN_SAVER);
        self.write_pad(1);
		self.write_u16(3);
		self.write_i16(timeout);
		self.write_i16(interval);
		self.write_u8(prefer_blanking.val());
		self.write_u8(allow_exposures.val());
		self.write_pad(2);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn get_screen_saver(&mut self) -> u16 {
        self.write_u8(protocol::OP_GET_SCREEN_SAVER);
        self.write_pad(1);
		self.write_u16(1);

        self.write_sequence(ServerReplyType::GetScreenSaver)
    }

    /** Tells the X Server to [TODO]
     * `family` must be one of HostFamily{Internet,DECnet,Chaos}
    */
    pub fn change_hosts(&mut self, address: &Vec<u8>, family: &HostFamily, mode: &ChangeHostMode) {
        self.write_u8(protocol::OP_CHANGE_HOSTS);
        self.write_u8(mode.val());
		let pad = self.write_dynamic_len(2, address.len());
		self.write_u8(family.val());
		self.write_pad(1);
		self.write_u16(address.len() as u16);
        self.write_raw(address);
		self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn list_hosts(&mut self) -> u16 {
        self.write_u8(protocol::OP_LIST_HOSTS);
        self.write_pad(1);
		self.write_u16(1);

        self.write_sequence(ServerReplyType::ListHosts)
    }

    /** Tells the X Server to [TODO] */
    pub fn set_access_control(&mut self, mode: bool) {
        self.write_u8(protocol::OP_SET_ACCESS_CONTROL);
        self.write_u8(if mode { 1 } else { 0 });
		self.write_u16(1);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn set_close_down_mode(&mut self, mode: &CloseDownMode) {
        self.write_u8(protocol::OP_SET_CLOSE_DOWN_MODE);
        self.write_u8(mode.val());
		self.write_u16(1);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `resource` = 0 = all temporary
     */
    pub fn kill_client(&mut self, resource: u32) {
        self.write_u8(protocol::OP_KILL_CLIENT);
        self.write_pad(1);
		self.write_u16(2);
		self.write_u32(resource);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn rotate_properties(&mut self, wid: u32, properties: &Vec<u32>, delta: i16) {
        self.write_u8(protocol::OP_ROTATE_PROPERTIES);
        self.write_pad(1);
		let pad = self.write_dynamic_len(3, properties.len() * 4);
		self.write_u32(wid);
		self.write_u16(properties.len() as u16);
		self.write_i16(delta);
        for prop in properties {
            self.write_u32(*prop);
        }
		self.write_pad_op(pad);

        self.write_request();
    }

    /** Tells the X Server to [TODO] */
    pub fn force_screen_saver(&mut self, reset: bool) {
        self.write_u8(protocol::OP_FORCE_SCREEN_SAVER);
        self.write_u8(if reset { 0 } else { 1 });
		self.write_u16(1);

        self.write_request();
    }

    /**
     * Tells the X Server to [TODO]
     * `map` must be 255 or less elements
     */
    pub fn set_pointer_mapping(&mut self, map: &Vec<u8>) -> u16 {
        self.write_u8(protocol::OP_SET_POINTER_MAPPING);
        self.write_u8(map.len() as u8);
		let pad = self.write_dynamic_len(1, map.len());
        self.write_raw(&map);
		self.write_pad_op(pad);

        self.write_sequence(ServerReplyType::SetPointerMapping)
    }

    /** Tells the X Server to [TODO] */
    pub fn get_pointer_mapping(&mut self) -> u16 {
        self.write_u8(protocol::OP_GET_POINTER_MAPPING);
        self.write_pad(1);
		self.write_u16(1);

        self.write_sequence(ServerReplyType::GetPointerMapping)
    }

    /**
     * Tells the X Server to [TODO]
     * `keycodes` must have 255 or less elements
     */
    pub fn set_modifier_mapping(&mut self, keycodes: &Vec<char>) -> u16 {
        self.write_u8(protocol::OP_SET_MODIFIER_MAPPING);
        self.write_u8(keycodes.len() as u8);
		let pad = self.write_dynamic_len(1, keycodes.len() * 8);
		self.write_pad_op(pad);

        self.write_sequence(ServerReplyType::SetModifierMapping)
    }

    /** Tells the X Server to [TODO] */
    pub fn get_modifier_mapping(&mut self) -> u16 {
        self.write_u8(protocol::OP_GET_MODIFIER_MAPPING);
        self.write_pad(1);
		self.write_u16(1);

        self.write_sequence(ServerReplyType::GetModifierMapping)
    }

    /** Tells the X Server to [TODO] */
    pub fn no_operation(&mut self, len: usize) {
        self.write_u8(protocol::OP_NO_OPERATION);
        self.write_pad(1);
		let pad = self.write_dynamic_len(1, len * 4);
        self.write_pad_op(len + pad);

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
        self.buf_out.write_all(buf).unwrap();
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
     * Writes a u16 defining the length specifier for a request, and returns the padding that would be required if there is one variable (in bytes).
     * base and len are both the number of bytes. So if you have x entries 8 bytes long, give len=x*8
     */
    fn write_dynamic_len(&mut self, base: u16, len: usize) -> usize {
        if len % 4 == 0 {
            self.write_u16(base + len as u16 / 4);
            return 0;
        } else {
            self.write_u16(base + len as u16 / 4 + 1);
            return 4 - len % 4;
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
     * Writes an i8 to the buffer.
     */
    fn write_i8(&mut self, input: i8) {
        self.write_u8(input as u8);
    }

    /**
     * Writes a char to the buffer.
     */
    fn write_char(&mut self, input: char) {
        self.write_u8(input as u8);
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
     * Writes a mask based off some u32 values.
     */
    fn write_mask_u32(&mut self, input: Vec<u32>) {
        let mut mask = 0;

        for val in input.iter() {
            mask |= val;
        }
        
        self.write_u32(mask);
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
    fn write_values<T: Value>(&mut self, values: &Vec<T>, mask_size: u8) {
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

        match mask_size {
            8 => {
                self.write_u8(value_mask as u8);
                self.write_pad(3)
            },
            16 => {
                self.write_u16(value_mask as u16);
                self.write_pad(2)
            },
            32 => self.write_u32(value_mask as u32),
            _ => panic!("Invalid mask size for write_values. Expected 8, 16, or 32 (bits).")
        };

        for i in order.iter() {
            values[*i].write(self);
        }
    }
}
