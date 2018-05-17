#!/usr/bin/env python3
# Parses the specification to generate read or write code.
# I don't use this often because I wanted human intuition behind my code, but
# since I have already written out the event stuff and I am using the same objects
# it should be fine to map out.
# I wrote this quickly with the goal of doing *most* things, not *all* things, so
# don't expect this to do everything. Also ignore the greedy regex.

import argparse
import re

reg_main_line = r"^([a-zA-Z0-9]+)$"
reg_request_len = r"^     2\s+(\S+)\s+request length$"
reg_reply_len = r"^     (\d+)\s+(\S+)\s+reply length$"
reg_standard = r"^     (\d+)\s+([\w\d]+)\s+(\S.*)$"
reg_unused = r"^     (\d+)\s+unused$" # Must be matched before enumerable
reg_data = r"^     (\d+)\s+data$" # Must be matched before enumerable
reg_enumerable = r"^     (\d+)\s+(\S.*)$"
reg_enum = r"^          (\d+)\s+[#\w]+$"

def read(path):
    with open(path, "r") as f:
        read = 0
        head = None
        old_head = None
        in_response = False
        param_names = []

        for line in f:
            while line.endswith("\n"):
                line = line[:-1]
            
            if not line:
                continue
            
            match = re.match(reg_main_line, line)
            if match:
                head = line
                in_response = False
                continue
            
            if line.startswith("▶"):
                if not head:
                    raise RuntimeError("Expected method name somewhere before response")

                if old_head:
                    print("Some(ServerReply::%s { %s })" % (old_head, ", ".join(param_names)))
                del param_names[:]
                old_head = head
                read = 0

                print()
                print(head)
                in_response = True
                continue
            
            if not in_response:
                continue

            match = re.match(reg_reply_len, line)
            if match:
                num = int(match.group(1))

                if num == 1:
                    base = "read_u8"
                    read += 1
                elif num == 2:
                    base = "read_u16"
                    read += 2
                elif num == 4:
                    base = "read_u32"
                    read += 4
                else:
                    print("//Could not parse length %d for %s" % (num, line))
                    continue

                if read > 8:
                    print("self.prep_read_extend(self.%s() as usize * 4);" % base)
                read += 2
                continue
            match = re.match(reg_standard, line)
            if match:
                num = int(match.group(1))
                symbol = "i" if match.group(2) in ("INT8", "INT16", "INT32") else "u"
                base = None

                if num == 1:
                    if match.group(2) == "BOOL":
                        base = "self.read_bool();"
                    else:
                        base = "self.read_%s8()" % symbol
                    read += 1
                elif num == 2:
                    base = "self.read_%s16()" % symbol
                    read += 2
                elif num == 4:
                    base = "self.read_%s32()" % symbol
                    read += 4
                else:
                    print("//Could not parse length %d for %s" % (num, line))
                    continue

                name = match.group(3).replace("-", "_").replace(" ", "_")
                if read > 8:
                    print("let %s = %s;" % (name, base))
                    param_names.append(name)
                continue
            match = re.match(reg_unused, line)
            if match:
                if read > 8:
                    print("self.read_pad(%d);" % int(match.group(1)))
                read += int(match.group(1))
                continue
            match = re.match(reg_data, line)
            if match:
                if read > 8:
                    print("let mut data = [0u8; %d];\nself.read_raw_buf(data_buf);" % int(match.group(1)))
                    param_names.append("data")
                read += int(match.group(1))
                continue
            match = re.match(reg_enumerable, line)
            if match:
                num = int(match.group(1))

                if num == 1:
                    base = "self.read_u8()"
                    read += 1
                elif num == 2:
                    base = "self.read_u16()"
                    read += 2
                elif num == 4:
                    base = "self.read_u32())"
                    read += 4
                else:
                    print("//Could not parse length %d for %s" % (num, line))
                    continue
                
                name = match.group(2).replace("-", "_").replace(" ", "_")
                if read > 8:
                    print("let %s = match ENUM.get(%s) {\n    Some(val) => val,\n    return None\n};" % (name, base))
                    param_names.append(name)
                continue
            match = re.match(reg_enum, line)
            if match:
                # Do nothing when writing. Might need to do stuff while reading tho.
                continue
            
            print("//Could not parse line %s" % line)

def write(path, is_event):
    with open(path, "r") as f:
        written = 0
        pad = False
        in_response = False

        for line in f:
            while line.endswith("\n"):
                line = line[:-1]

            if not line:
                continue
            
            match = re.match(reg_main_line, line)
            if match:
                if pad:
                    print("self.write_pad_op(pad);")
                if is_event and written != -1 and written != 32:
                    print("WARNING: Above statement did not add up to 32 bytes (was %d instead)" % written)

                print()
                print(line)
                written = 0
                pad = False
                in_response = False
                continue
            
            if line.startswith("▶"):
                in_response = True
                continue

            if in_response:
                continue

            match = re.match(reg_request_len, line)
            if match:
                try:
                    l = int(match.group(1))
                    print("self.write_u16(%d);" % l)
                except:
                    print("let pad = self.write_dynamic_len(TODO_BASE, TODO_EXTRA); // TODO: Fill this in from %s" % match.group(1))
                    pad = True
                written += 2
                continue
            match = re.match(reg_standard, line)
            if match:
                num = int(match.group(1))
                symbol = "i" if match.group(2) in ("INT8", "INT16", "INT32") else "u"

                if num == 1:
                    if match.group(2) == "BOOL":
                        print("self.write_bool(%s);" % match.group(3).replace("-", "_").replace(" ", "_"))
                    else:
                        print("self.write_%s8(%s);" % (symbol, match.group(3).replace("-", "_").replace(" ", "_")))
                    written += 1
                elif num == 2:
                    print("self.write_%s16(%s);" % (symbol, match.group(3).replace("-", "_").replace(" ", "_")))
                    written += 2
                elif num == 4:
                    print("self.write_%s32(%s);" % (symbol, match.group(3).replace("-", "_").replace(" ", "_")))
                    written += 4
                else:
                    print("//Could not parse length %d for %s" % (num, line))

                continue
            match = re.match(reg_unused, line)
            if match:
                print("self.write_pad(%d);" % int(match.group(1)))
                written += int(match.group(1))
                continue
            match = re.match(reg_data, line)
            if match:
                print("self.write_raw(TODO_DATA); // TODO: Limit length to %d" % int(match.group(1)))
                written += int(match.group(1))
                continue
            match = re.match(reg_enumerable, line)
            if match:
                num = int(match.group(1))

                if num == 1:
                    print("self.write_u8(%s.val());" % match.group(2).replace("-", "_").replace(" ", "_"))
                    written += 1
                elif num == 2:
                    print("self.write_u16(%s.val());" % match.group(2).replace("-", "_").replace(" ", "_"))
                    written += 2
                elif num == 4:
                    print("self.write_u32(%s.val());" % match.group(2).replace("-", "_").replace(" ", "_"))
                    written += 4
                else:
                    print("//Could not parse length %d for %s" % (num, line))

                continue
            match = re.match(reg_enum, line)
            if match:
                # Do nothing when writing. Might need to do stuff while reading tho.
                continue
            
            print("//Could not parse line %s" % line)
                

if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description = "Parse parts of the specification to RUST code. Ie: https://www.x.org/releases/X11R7.7/doc/xproto/x11protocol.html#Encoding::Events",
    )
    parser.add_argument("read|write", help="Is your action reading or writing?")
    parser.add_argument("path", help="Portion of the specification to parse")
    parser.add_argument("--event", action="store_true", help="If set, will warn if section is not 32 bytes (required event length)")
    args = parser.parse_args().__dict__

    if args["read|write"].lower() == "read":
        read(args["path"])
    elif args["read|write"].lower() == "write":
        write(args["path"], args["event"])
    else:
        print("Error: Argument 1 must be one of 'read' or 'write'")
