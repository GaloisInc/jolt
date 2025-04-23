#![cfg_attr(feature = "guest", no_std)]

use serde::{Deserialize, Serialize};
//#[macro_use]
//extern crate serde_big_array;

use core::{fmt, num};
use serde::{
    de::{SeqAccess, Visitor},
    ser::{SerializeTuple, Serializer},
    Deserializer,
};

pub const N: usize = 100000;
#[derive(Clone)]
pub struct MyArray([u8; N]);

impl Default for MyArray {
    fn default() -> Self {
        MyArray([0u8; N])
    }
}

impl Serialize for MyArray {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(N)?;
        for &b in &self.0 {
            tup.serialize_element(&b)?;
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for MyArray {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor;

        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = MyArray;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an array of length N")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<MyArray, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut arr = [0u8; N];
                for i in 0..N {
                    arr[i] = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                // ensure there are no extra elements
                if let Some(_) = seq.next_element::<u8>()? {
                    return Err(serde::de::Error::invalid_length(N + 1, &self));
                }
                Ok(MyArray(arr))
            }
        }

        deserializer.deserialize_tuple(N, ArrayVisitor)
    }
}

// #[jolt::provable(max_input_size = 10000, max_output_size = 10000)]

// Let define a type for the buffer of u8 containing images, with the size. The type has to be serializable.
// The type should just be a rename of the array:
//#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
//pub struct ImageBuffer(#[serde(with = "BigArray")] pub [u8; 1000]);

impl MyArray {
    pub fn new(data: &[u8]) -> Self {
        let mut buffer = [0u8; N];
        let len = data.len();
        if len > N {
            panic!("Data is too long");
        }
        for i in 0..len {
            buffer[i] = data[i];
        }
        for i in len..N {
            buffer[i] = 0;
        }

        MyArray(buffer)
    }
}

#[jolt::provable]
pub fn jpeg2000(data: MyArray, len: usize) -> bool {
    // Get a slice of MyArray of size len
    let data_slice = &data.0[0..len];

    return validate_jpeg2k(data_slice);
}

const mem_size_of_u32: u32 = 4u32;

pub fn validate_jpeg2k(data: &[u8]) -> bool {
    let mut pos: usize = 0; // Current index in the image
    let mut res: bool;
    /* Signature Box */
    // The signature box size is 12 bytes long [0x 0000 000C 6A50 0D0A 870A]
    let signature_box_size: [u8; 4] = [0x00, 0x00, 0x00, 0x0C];
    let signature_box_content: [u8; 8] = [0x6A, 0x50, 0x20, 0x20, 0x0D, 0x0A, 0x87, 0x0A];
    // Box size
    if data[pos..pos + 4] != signature_box_size {
        return false;
    }
    if data[pos + 4..pos + 12] != signature_box_content {
        return false;
    }
    // Add signature box size to pos
    pos += 12;
    /* FTYPE Box */
    // Validate Ftype box type
    if data[pos + 4..pos + 8] != [0x66, 0x74, 0x79, 0x70] {
        return false;
    }

    //Validate Ftype box size, skip minor version field if needed.
    let skip_size: u32 = 4;
    let ftype_size: u32 = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
    if ftype_size - (8 as u32) < skip_size + (mem_size_of_u32)
    // mem::size_of::<u32>() as u32 // NOT SUPPORTED JOLT
    //size of header without type and length fields
    {
        return false;
    }

    if 0 != ftype_size % (mem_size_of_u32) {
        // mem::size_of::<u32>() as u32 // NOT SUPPORTED JOLT
        return false;
    }

    //subVersion validation: First 4 bytes of the content = 0x6A703220
    if data[pos + 8..pos + 12] != [0x6A, 0x70, 0x32, 0x20] {
        return false;
    }
    pos += ftype_size as usize;

    // Skip boxes until we reach jp2 header box
    let header_box_type: [u8; 4] = [0x6A, 0x70, 0x32, 0x68];
    let mut header_size: u32 = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
    while data[pos + 4..pos + 8] != header_box_type {
        pos += header_size as usize;
        header_size = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
    }
    pos += 8; // Already read length and type in the loop

    /* Header Box */
    // Validate header box size
    let min_header_box_len: u32 = 14;
    if header_size < min_header_box_len {
        return false;
    }
    /* ihdr */
    //next sub-box is ihdr - validate size and type
    let ihdr_type: [u8; 4] = [0x69, 0x68, 0x64, 0x72];
    let ihdr_len: u32 = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
    if ihdr_len != min_header_box_len + (8 as u32)
    // Size of contents + (size,type) fields
    {
        return false;
    }
    if data[pos + 4..pos + 8] != ihdr_type {
        return false;
    }
    pos += 8;
    pos += min_header_box_len as usize; //skip content of header
                                        //header box  - (size+type of header box = 8) - ihdr ( = 8 + min_header_len)
    let mut remaining_header_box_size: u32 = header_size - min_header_box_len - (16 as u32);

    /* Inside Header Box - go over remaining box size and parse the following boxes, each according to its own parser */
    //count occurence of each block type
    let mut pclr_box_counter = 0; //Should be between 0,1
    let mut color_box_counter = 0; //Should be between 1,10
    let mut cmap_box_counter = 0; //Should be between 0,1
    let mut cdef_box_counter = 0; //should be between 0,1

    let mut color_meth: u8; // The first byte after size+type int the color box - its values are 1 or 2
    let mut enum_cs: u32; //Value = 16,17 and exists only if color_meth = 2

    let mut pclr_entries_num: u16; //in range 1-1024, 2 bytes big endian
    let mut pclr_component_num: u8;
    let mut pclr_num_bytes_colors: u32; //C_(ji) - colors = num_entries*compoents
    let mut cmap_num_channels_description: u32;
    let mut cdef_num_channel_discription: u16;

    let color_box_type: [u8; 4] = [0x63, 0x6F, 0x6C, 0x72];
    let pclr_box_type: [u8; 4] = [0x70, 0x63, 0x6C, 0x72];
    let cmap_box_type: [u8; 4] = [0x63, 0x6D, 0x61, 0x70];
    let cdef_box_type: [u8; 4] = [0x63, 0x64, 0x65, 0x66];
    let res_superbox_type: [u8; 4] = [0x72, 0x65, 0x73, 0x20];
    let uuid_box_type: [u8; 4] = [0x75, 0x75, 0x69, 0x64];
    let uuid_info_superbox: [u8; 4] = [0x75, 0x69, 0x6e, 0x66];

    //loop on the rest of the boxes, parse by type
    while remaining_header_box_size > 0 {
        let box_size: u32 = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
        if data[pos + 4..pos + 8] == color_box_type {
            /* Color Box */
            //Validate size
            if box_size - (8 as u32) < 5 as u32 {
                return false;
            }
            let skip_size: usize;
            if color_box_counter == 0 {
                color_meth = data[pos + 8];
                if color_meth < 1 || color_meth > 2 {
                    return false;
                }

                if color_meth == (2 as u8) {
                    //RESTRICTED_ICC
                    skip_size = (mem_size_of_u32 as usize) * (4 as usize); //skip ICC size, CMM type signature, version, class signature // // mem::size_of::<u32>() as u32 // NOT SUPPORTED JOLT
                    enum_cs = u32::from_be_bytes(
                        data[pos + skip_size + 9..pos + skip_size + 13]
                            .try_into()
                            .unwrap(),
                    );
                    if enum_cs < 16 || enum_cs > 17 {
                        return false;
                    }
                }
            }
            color_box_counter += 1;
        } else if data[pos + 4..pos + 8] == pclr_box_type {
            /* PCLR Box */
            //Validate size
            if box_size - (8 as u32) < 5 as u32
            //box_size without size and type
            {
                return false;
            }
            pclr_entries_num = u16::from_be_bytes(data[pos + 8..pos + 10].try_into().unwrap());
            if pclr_entries_num > 1024 {
                return false;
            }
            pclr_component_num = data[pos + 10];
            pclr_num_bytes_colors = (pclr_entries_num as u32) * (pclr_component_num as u32);
            if pclr_num_bytes_colors > 16777216 {
                return false;
            }
            pclr_box_counter += 1;
        } else if data[pos + 4..pos + 8] == cmap_box_type {
            /* Cmap_BOX */
            // Validate size
            if box_size - (8 as u32) < 4 as u32
            //box_size without size and type
            {
                return false;
            }
            if 0 != box_size % (4 as u32) {
                return false;
            }
            cmap_num_channels_description = (box_size - (8 as u32)) / (4 as u32);
            for i in 0..(cmap_num_channels_description as usize) {
                let mtyp: u8 = data[pos + 8 + 4 * i + 2];
                let palette_column: u8 = data[pos + 8 + 4 * i + 3];
                // mtyp == 0 means direct use, 1 means palette mapping
                if mtyp == 0 {
                    if palette_column != 0 {
                        return false;
                    }
                }
            }
            cmap_box_counter += 1;
        } else if data[pos + 4..pos + 8] == cdef_box_type {
            /* CDEF_BOX */
            if box_size - (8 as u32) < (8 as u32) {
                return false;
            }
            if (box_size - (8 as u32) - (2 as u32)) % (6 as u32) != 0
            //(box_size - size,type -num_channel_size)%channel_def_size
            {
                return false;
            }
            cdef_num_channel_discription =
                u16::from_be_bytes(data[pos + 8..pos + 10].try_into().unwrap());
            for i in 0..(cdef_num_channel_discription as usize) {
                let channel_index: u16 = u16::from_be_bytes(
                    data[pos + 10 + 6 * i..pos + 10 + 6 * i + 2]
                        .try_into()
                        .unwrap(),
                );
                if channel_index >= cdef_num_channel_discription {
                    return false;
                }
            }
            cdef_box_counter += 1;
        } else if data[pos + 4..pos + 8] == res_superbox_type {
            /* Resolution SuperBox */
            let mut num_capture_resolution_box = 0;
            let mut num_default_display_resolution_box = 0;

            let first_subheader_size: u32 =
                u32::from_be_bytes(data[pos + 8..pos + 12].try_into().unwrap());
            let first_box_type: &[u8] = &data[pos + 12..pos + 16];
            if first_box_type == [0x72, 0x65, 0x73, 0x63] {
                num_capture_resolution_box += 1;
            } else if first_box_type == [0x72, 0x65, 0x73, 0x64] {
                num_default_display_resolution_box += 1;
            }

            if box_size != (8 as u32) + first_subheader_size {
                let second_subheader_size: u32 = u32::from_be_bytes(
                    data[pos + 8 + (first_subheader_size as usize)
                        ..pos + 12 + (first_subheader_size as usize)]
                        .try_into()
                        .unwrap(),
                );
                // The size of the box is the sum over all sub boxes and this header type and size.
                if box_size != 8 + first_subheader_size + second_subheader_size {
                    return false;
                }
                let second_box_type: &[u8] = &data[pos + 12..pos + 16];
                if second_box_type == [0x72, 0x65, 0x73, 0x63] {
                    num_capture_resolution_box += 1;
                } else if second_box_type == [0x72, 0x65, 0x73, 0x64] {
                    num_default_display_resolution_box += 1;
                }
            }
            // At leat one box should appear, and every box appears at most once.
            if num_capture_resolution_box > 1 {
                return false;
            }
            if num_default_display_resolution_box > 1 {
                return false;
            }
            if num_default_display_resolution_box + num_capture_resolution_box <= 0 {
                return false;
            }
        } else if data[pos + 4..pos + 8] == uuid_box_type {
            /* UUID Box */
            // pass
        } else if data[pos + 4..pos + 8] == uuid_info_superbox {
            /* UUID Info SuperBox */
            // First Box is UUID List Box
            let first_box_len = u32::from_be_bytes(data[pos + 8..pos + 12].try_into().unwrap());
            if data[pos + 4 + 8..pos + 8 + 8] != [0x75, 0x6c, 0x73, 0x74] {
                return false;
            }
            // Second Box is Data Entry URL box
            if data[pos + 8 + (first_box_len as usize)..pos + 12 + (first_box_len as usize)]
                != [0x75, 0x72, 0x6c, 0x20]
            {
                return false;
            }
        } else {
            return false;
        }

        if pclr_box_counter > 1 {
            return false;
        }
        if cmap_box_counter > 1 {
            return false;
        }
        if cdef_box_counter > 1 {
            return false;
        }
        if color_box_counter > 10 {
            return false;
        }

        pos += box_size as usize;
        remaining_header_box_size -= box_size;
    }

    if color_box_counter <= 0 {
        return false;
    }
    if remaining_header_box_size != 0 {
        return false;
    }
    /* Done parsing header box */
    let codestream_box_type: [u8; 4] = [0x6A, 0x70, 0x32, 0x63];
    //skip boxes until we reach codestream box
    let mut code_stream_box_size: u32 = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
    while data[pos + 4..pos + 8] != codestream_box_type {
        pos += code_stream_box_size as usize;
        code_stream_box_size = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
    }

    /* CodeStream Box */
    //check if the codestream box is the last box of the file
    if code_stream_box_size == 0 {
        code_stream_box_size = (data.len() as u32) - (pos as u32);
    }
    // check if the length of the codestream doesn't exceeds
    if code_stream_box_size > (data.len() as u32) - (pos as u32) {
        return false;
    }
    pos += 8; //skip header

    /* Main Header */
    /* SOC Marker */
    let soc_marker_first_byte: u8 = data[pos];
    if soc_marker_first_byte != 255 as u8 {
        return false;
    }
    if data[pos + 1] != 79 as u8
    //Marker Type
    {
        return false;
    }
    pos += 2;

    /* SIZ Marker */
    let siz_marker_first_byte: u8 = data[pos];
    if siz_marker_first_byte != 255 as u8 {
        return false;
    }
    if data[pos + 1] != 81 as u8 {
        return false;
    }
    pos += 2;

    let siz_marker_size: u16 = u16::from_be_bytes(data[pos..pos + 2].try_into().unwrap());
    let ca: u16 = u16::from_be_bytes(data[pos + 2..pos + 4].try_into().unwrap());
    if ca != 0 as u16 {
        return false;
    }
    let image_width: u32 = u32::from_be_bytes(data[pos + 4..pos + 8].try_into().unwrap());
    let image_height: u32 = u32::from_be_bytes(data[pos + 8..pos + 12].try_into().unwrap());
    let image_horizontal_offset: u32 =
        u32::from_be_bytes(data[pos + 12..pos + 16].try_into().unwrap());
    let image_vertical_offset: u32 =
        u32::from_be_bytes(data[pos + 16..pos + 20].try_into().unwrap());
    let image_tile_width: u32 = u32::from_be_bytes(data[pos + 20..pos + 24].try_into().unwrap());
    let image_tile_height: u32 = u32::from_be_bytes(data[pos + 24..pos + 28].try_into().unwrap());
    let image_first_tile_horizontal_offset =
        u32::from_be_bytes(data[pos + 28..pos + 32].try_into().unwrap());
    let image_first_tile_vertical_offset =
        u32::from_be_bytes(data[pos + 32..pos + 36].try_into().unwrap());

    if image_first_tile_horizontal_offset != 0 as u32 {
        return false;
    }
    if image_first_tile_vertical_offset != 0 as u32 {
        return false;
    }
    if image_tile_height == 0 {
        return false;
    }
    if image_tile_width == 0 {
        return false;
    }
    if image_tile_height > image_height {
        return false;
    }
    if image_tile_width > image_width {
        return false;
    }
    if image_horizontal_offset >= image_width {
        return false;
    }
    if image_vertical_offset >= image_height {
        return false;
    }
    let image_num_components = u16::from_be_bytes(data[pos + 36..pos + 38].try_into().unwrap());
    if (siz_marker_size as u16) != image_num_components * (3 as u16) + (38 as u16) {
        return false;
    }
    let rgb_num_components: u16 = 3;
    let rgba_num_components: u16 = 4;
    let grayscale_num_components: u16 = 1;
    if image_num_components != rgb_num_components
        && image_num_components != rgba_num_components
        && image_num_components != grayscale_num_components
    //grayscale or RGB
    {
        return false;
    }
    for i in 0..(image_num_components as usize) {
        let bit_depth: u8 = data[pos + 38 + i * 3];
        if bit_depth < 1 || bit_depth > 38 {
            return false;
        }
    }

    //calc num of tiles
    // before:
    // let num_x_tiles: u32 = (((image_width - image_first_tile_horizontal_offset) as f32)
    //     / (image_tile_width as f32))
    //     .ceil() as u32;
    // let num_y_tiles: u32 = (((image_height - image_first_tile_vertical_offset) as f32)
    //     / (image_tile_height as f32))
    //     .ceil() as u32;

    let num_of_tiles: u32;
    let rem_x = image_width
        .checked_sub(image_first_tile_horizontal_offset)
        .unwrap_or(0);
    let rem_y = image_height
        .checked_sub(image_first_tile_vertical_offset)
        .unwrap_or(0);

    // integer ceiling‐div: (a + b − 1) / b
    let num_x_tiles = (rem_x + image_tile_width - 1) / image_tile_width;
    let num_y_tiles = (rem_y + image_tile_height - 1) / image_tile_height;
    num_of_tiles = num_x_tiles * num_y_tiles;
    pos += siz_marker_size as usize;

    /* Filter all markers in the header until we reach SOT Marker */
    let sot_market_type: u8 = 144;
    let cod_marker_type: u8 = 82;
    let coc_marker_type: u8 = 83;
    let rgn_marker_type: u8 = 94;
    let qcd_marker_type: u8 = 92;
    let qcc_marker_type: u8 = 93;
    let poc_marker_type: u8 = 95;
    let tlm_marker_type: u8 = 85;
    let ppm_marker_type: u8 = 86;
    let crg_marker_type: u8 = 89;
    let com_marker_type: u8 = 100;
    let ppt_marker_type: u8 = 97;
    let plt_marker_type: u8 = 88;
    let mut cod_marker_counter: u8 = 0;
    let mut coc_marker_counter: u8 = 0;
    let mut qcd_marker_counter: u8 = 0;
    let mut qcc_marker_counter: u8 = 0;
    let mut poc_marker_counter: u8 = 0;
    let mut quantum_num_decomposition_levels: u8 = 0; // Will be equal to qcc_num_decomp_levls if defined, or to qcd_num_decomp_levls if not.
    let mut num_decomposition_levels: u8 = 0; // Will be equal to coc_num_decomp_levls if defined, or to cod_num_decomp_levls if not.
    let mut _quantization_style: u8 = 0;

    // Read first marker outside of the loop
    let mut marker_type: u8;
    let mut marker_length: u16;
    (res, marker_type, marker_length, pos) = get_marker_type_and_length(&data, pos);
    if !res {
        return false;
    }
    while marker_type != sot_market_type {
        if marker_type == cod_marker_type {
            /* COD Marker */
            // println!("inside cod"); // NO SUPPORTED JOLT
            let cod_num_decomposition_levls: u8;
            if cod_marker_counter != 0 {
                return false;
            }
            (res, cod_num_decomposition_levls, pos) = parse_cod(&data, pos, marker_length);
            if !res {
                return false;
            }
            if coc_marker_counter == 0 {
                num_decomposition_levels = cod_num_decomposition_levls;
            } //coc overrides cod
            cod_marker_counter += 1;
        } else if marker_type == coc_marker_type {
            /* COC Marker */
            // println!("Inside coc"); // NO SUPPORTED JOLT
            let coc_num_decomposition_levls: u8;
            if (coc_marker_counter as u16) >= image_num_components {
                return false;
            }
            (res, coc_num_decomposition_levls, pos) =
                parse_coc(&data, pos, marker_length, image_num_components);
            if !res {
                return false;
            }
            coc_marker_counter += 1 as u8;
            num_decomposition_levels = coc_num_decomposition_levls;
        } else if marker_type == rgn_marker_type {
            /* RGN Marker */
            (res, pos) = parse_rgn(&data, pos, marker_length, image_num_components);
            if !res {
                return false;
            }
        } else if marker_type == qcd_marker_type {
            /* QCD Marker */
            // println!("inside qcd"); // NO SUPPORTED JOLT
            let qcd_num_decomposition_levls: u8;
            let qcd_quantization_style: u8;
            if qcc_marker_counter != 0 {
                return false;
            }
            if qcd_marker_counter != 0 {
                return false;
            }
            (
                res,
                qcd_num_decomposition_levls,
                qcd_quantization_style,
                pos,
            ) = parse_qcd(&data, pos, marker_length);
            if !res {
                return false;
            }
            qcd_marker_counter += 1 as u8;
            if qcc_marker_counter == 0 {
                _quantization_style = qcd_quantization_style;
                quantum_num_decomposition_levels = qcd_num_decomposition_levls;
            }
        } else if marker_type == qcc_marker_type {
            /* QCC */
            let qcc_num_decomposition_levls: u8;
            let qcc_quantization_style: u8;
            if qcc_marker_counter != 0 {
                return false;
            }
            (
                res,
                qcc_num_decomposition_levls,
                qcc_quantization_style,
                pos,
            ) = parse_qcc(&data, pos, marker_length, image_num_components);
            if !res {
                return false;
            }
            _quantization_style = qcc_quantization_style;
            quantum_num_decomposition_levels = qcc_num_decomposition_levls;
            qcc_marker_counter += 1 as u8;
        } else if marker_type == poc_marker_type {
            /* POC Marker */
            // println!("inside poc"); // NO SUPPORTED JOLT
            if poc_marker_counter != 0 {
                return false;
            }
            (res, pos) = parse_poc(&data, pos, marker_length, image_num_components);
            if !res {
                return false;
            }
            poc_marker_counter += 1 as u8;
        } else if marker_type == tlm_marker_type {
            /* TLM Marker */
            (res, pos) = parse_tlm(&data, pos, marker_length);
            if !res {
                return false;
            }
        } else if marker_type == ppm_marker_type {
            /* PPM */
            if marker_length < 7 {
                return false;
            }
        } else if marker_type == crg_marker_type {
            /* CRG */
            if marker_length < 6 {
                return false;
            }
        } else if marker_type == com_marker_type {
            /* COM */
            (res, pos) = parse_com(&data, pos, marker_length);
            if !res {
                return false;
            }
        } else {
            return false;
        }

        (res, marker_type, marker_length, pos) = get_marker_type_and_length(&data, pos);
        if !res {
            return false;
        }
    }
    if quantum_num_decomposition_levels != num_decomposition_levels {
        return false;
    }
    if cod_marker_counter != 1 {
        return false;
    }
    if qcd_marker_counter != 1 {
        return false;
    }

    /* Parse tile parts */
    // There must be at least one tile part.
    let sod_market_type: u8 = 147;
    loop {
        /* Parse tile part */
        // Parse SOT marker
        if marker_length != 10 {
            return false;
        }
        let tile_index: u16 = u16::from_be_bytes(data[pos..pos + 2].try_into().unwrap());
        if (tile_index as u32) >= num_of_tiles {
            return false;
        }
        let mut len_of_tile_part: u32 =
            u32::from_be_bytes(data[pos + 2..pos + 6].try_into().unwrap());
        let pos_at_begin_tile_part: usize = (pos as usize) - 4; // beginning of sot
        if len_of_tile_part == 0 {
            len_of_tile_part = (data.len() as u32) - ((pos) as u32) + (4 as u32);
        } //len from first byte of SOT
        let tile_part_index: u8 = data[pos + 6];
        if tile_part_index >= 255 {
            return false;
        }
        pos += 8;
        // end of sot marker
        // Parse the tile_part_header markers until reach SOD - start of data marker
        cod_marker_counter = 0;
        coc_marker_counter = 0;
        qcd_marker_counter = 0;
        qcc_marker_counter = 0;
        poc_marker_counter = 0;

        (res, marker_type, marker_length, pos) = get_marker_type_and_length(&data, pos);
        if !res {
            return false;
        }
        while marker_type != sod_market_type {
            if marker_type == cod_marker_type && tile_part_index == 0 {
                /* COD Marker */
                // println!("inside cod"); // NO SUPPORTED JOLT
                let cod_num_decomposition_levls: u8;
                if cod_marker_counter != 0 {
                    return false;
                }
                (res, cod_num_decomposition_levls, pos) = parse_cod(&data, pos, marker_length);
                if !res {
                    return false;
                }
                if coc_marker_counter == 0 {
                    num_decomposition_levels = cod_num_decomposition_levls;
                } //coc overrides cod
                cod_marker_counter += 1;
            } else if marker_type == coc_marker_type && tile_part_index == 0 {
                /* COC Marker */
                // println!("Inside coc"); // NO SUPPORTED JOLT
                let coc_num_decomposition_levls: u8;
                if (coc_marker_counter as u16) >= image_num_components {
                    return false;
                }
                (res, coc_num_decomposition_levls, pos) =
                    parse_coc(&data, pos, marker_length, image_num_components);
                if !res {
                    return false;
                }
                coc_marker_counter += 1 as u8;
                num_decomposition_levels = coc_num_decomposition_levls;
            } else if marker_type == rgn_marker_type && tile_part_index == 0 {
                /* RGN Marker */
                // println!("tile rgn"); // NO SUPPORTED JOLT
                (res, pos) = parse_rgn(&data, pos, marker_length, image_num_components);
                if !res {
                    return false;
                }
            } else if marker_type == qcd_marker_type && tile_part_index == 0 {
                /* QCD Marker */
                // println!("inside qcd"); // NO SUPPORTED JOLT
                let qcd_num_decomposition_levls: u8;
                let qcd_quantization_style: u8;
                if qcc_marker_counter != 0 {
                    return false;
                }
                if qcd_marker_counter != 0 {
                    return false;
                }
                (
                    res,
                    qcd_num_decomposition_levls,
                    qcd_quantization_style,
                    pos,
                ) = parse_qcd(&data, pos, marker_length);
                if !res {
                    return false;
                }
                qcd_marker_counter += 1 as u8;
                if qcc_marker_counter == 0 {
                    _quantization_style = qcd_quantization_style;
                    quantum_num_decomposition_levels = qcd_num_decomposition_levls;
                }
            } else if marker_type == qcc_marker_type && tile_part_index == 0 {
                /* QCC */
                // println!("tile qcc"); // NO SUPPORTED JOLT
                let qcc_num_decomposition_levls: u8;
                let qcc_quantization_style: u8;
                if qcc_marker_counter != 0 {
                    return false;
                }
                (
                    res,
                    qcc_num_decomposition_levls,
                    qcc_quantization_style,
                    pos,
                ) = parse_qcc(&data, pos, marker_length, image_num_components);
                if !res {
                    return false;
                }
                _quantization_style = qcc_quantization_style;
                quantum_num_decomposition_levels = qcc_num_decomposition_levls;
                qcc_marker_counter += 1 as u8;
            } else if marker_type == poc_marker_type {
                /* POC Marker */
                // println!("inside poc"); // NO SUPPORTED JOLT
                if poc_marker_counter != 0 {
                    return false;
                }
                (res, pos) = parse_poc(&data, pos, marker_length, image_num_components);
                if !res {
                    return false;
                }
                poc_marker_counter += 1 as u8;
            } else if marker_type == com_marker_type {
                /* COM */
                // println!("tile com"); // NO SUPPORTED JOLT
                (res, pos) = parse_com(&data, pos, marker_length);
                if !res {
                    return false;
                }
            } else if marker_type == ppt_marker_type {
                /* PPT */
                // println!("tile ppt"); // NO SUPPORTED JOLT
                if marker_length < 4 {
                    return false;
                }
                pos += (marker_length as usize) - 2;
            } else if marker_type == plt_marker_type {
                /* PLT */
                // println!("tile plt"); // NO SUPPORTED JOLT
                if marker_length < 4 {
                    return false;
                }
                pos += (marker_length as usize) - 2;
            } else {
                return false;
            }
            (res, marker_type, marker_length, pos) = get_marker_type_and_length(&data, pos);
            if !res {
                return false;
            }
        }

        /* Parse Packet Header in tile part */
        // SOP (Start of packet) Marker is optional
        if data[pos] == 0xFF && data[pos + 1] == 0x93 {
            let sop_len = u16::from_be_bytes(data[pos + 2..pos + 4].try_into().unwrap());
            if sop_len != 4 {
                return false;
            }
            pos += 4;
        }

        // check no headers are in data
        // let mut bad_positions: Vec<usize> = Vec::new();  // NOT SUPPORTED JOLT
        for i in
            (pos_at_begin_tile_part as usize)..(pos_at_begin_tile_part + len_of_tile_part as usize)
        {
            let next_byte: u8 = data[i];
            if next_byte == 255 as u8 {
                if (i + 1 < pos_at_begin_tile_part + (len_of_tile_part as usize))
                    && data[i + 1] > 143
                {
                    // bad_positions.push(i);  // NOT SUPPORTED JOLT
                }
            }
        }
        pos = pos_at_begin_tile_part + (len_of_tile_part as usize);

        /* Read Marker */
        (res, marker_type, marker_length, pos) = get_marker_type_and_length(&data, pos);
        if !res {
            return false;
        }
        // println!("inside loop tile part"); // NO SUPPORTED JOLT
        if marker_type != sot_market_type {
            break;
        }
    }
    true
}

/* Parsers */

pub fn get_marker_type_and_length(data: &[u8], pos: usize) -> (bool, u8, u16, usize) {
    let mut local_pos: usize = pos;
    let mut res: bool = true;
    let mut marker_length: u16 = 0;
    let marker_first_byte: u8 = data[local_pos];
    if marker_first_byte != 255 as u8 {
        res = false;
    }
    let marker_type: u8 = data[local_pos + 1]; //Marker Type
    local_pos += 2;
    if marker_type != 217 && marker_type != 147 {
        // Not EOC or SOD Markers
        marker_length = u16::from_be_bytes(data[local_pos..local_pos + 2].try_into().unwrap());
        local_pos += 2;
    }
    (res, marker_type, marker_length, local_pos)
}

pub fn parse_com(data: &[u8], pos: usize, marker_length: u16) -> (bool, usize) {
    let mut local_pos: usize = pos;
    if marker_length < 5 {
        return (false, local_pos);
    }
    let rcom: u16 = u16::from_be_bytes(data[local_pos..local_pos + 2].try_into().unwrap());
    if rcom > 1 {
        return (false, local_pos);
    }
    local_pos += (marker_length as usize) - 2;
    (true, local_pos)
}

pub fn parse_tlm(data: &[u8], pos: usize, marker_length: u16) -> (bool, usize) {
    let mut local_pos: usize = pos;
    if marker_length < 6 {
        return (false, local_pos);
    }
    let stlm: u8 = data[local_pos + 1];
    if stlm & 0x0F != 0 || stlm & 0x80 != 0 {
        return (false, local_pos);
    }

    local_pos += (marker_length as usize) - 2;
    (true, local_pos)
}

pub fn parse_poc(
    data: &[u8],
    pos: usize,
    marker_length: u16,
    image_num_components: u16,
) -> (bool, usize) {
    let mut local_pos: usize = pos;
    if marker_length < (9 as u16) {
        return (false, local_pos);
    }
    let num_prog_order_change: u16 = (marker_length - (2 as u16))
        / ((7 as u16) + (2 as u16) * (image_num_components / (256 as u16)));
    for i in 0..((num_prog_order_change) as usize) {
        let prog_order: u8 = data[pos + (i as usize)];
        if prog_order >= 34 {
            return (false, local_pos);
        }
    }
    local_pos += num_prog_order_change as usize;
    (true, local_pos)
}

pub fn parse_qcc(
    data: &[u8],
    pos: usize,
    marker_length: u16,
    image_num_components: u16,
) -> (bool, u8, u8, usize) {
    let mut local_pos: usize = pos;
    let qcc_quantization_style: u8;
    let mut qcc_num_decomposition_levls: u8 = 0;
    if marker_length < 5 || marker_length > 199 {
        return (false, qcc_num_decomposition_levls, 0 as u8, local_pos);
    }
    if image_num_components < 257 {
        local_pos += 1;
    }
    local_pos += 1; // If num of components > 256, the index is 2 bytes, otherwise it is 1 byte.
    qcc_quantization_style = data[local_pos];
    if qcc_quantization_style & 0x03 == 3 {
        return (
            false,
            qcc_num_decomposition_levls,
            qcc_quantization_style,
            local_pos,
        );
    }
    // We are interested in reversible transorm only
    if qcc_quantization_style & 0x03 != 0 || qcc_quantization_style & 0x0E != 0 {
        return (
            false,
            qcc_num_decomposition_levls,
            qcc_quantization_style,
            local_pos,
        );
    }
    // The ifdef also containes cases where the transform is not reversible, for clarity
    if qcc_quantization_style & 0x03 == 0 && image_num_components < 257 {
        qcc_num_decomposition_levls = ((marker_length - (5 as u16)) / (3 as u16)) as u8;
    } else if qcc_quantization_style & 0x03 == 0 && image_num_components >= 257 {
        qcc_num_decomposition_levls = ((marker_length - (6 as u16)) / (3 as u16)) as u8;
    } else if qcc_quantization_style & 0x03 == 1 && image_num_components < 257 {
        if marker_length != (6 as u16) {
            return (
                false,
                qcc_num_decomposition_levls,
                qcc_quantization_style,
                local_pos,
            );
        }
        qcc_num_decomposition_levls = 0;
    } else if qcc_quantization_style & 0x03 == 1 && image_num_components >= 257 {
        if marker_length != (7 as u16) {
            return (
                false,
                qcc_num_decomposition_levls,
                qcc_quantization_style,
                local_pos,
            );
        }
        qcc_num_decomposition_levls = 0;
    } else if qcc_quantization_style & 0x03 == 2 && image_num_components < 257 {
        qcc_num_decomposition_levls = ((marker_length - (6 as u16)) / (6 as u16)) as u8;
    } else if qcc_quantization_style & 0x03 == 2 && image_num_components >= 257 {
        qcc_num_decomposition_levls = ((marker_length - (7 as u16)) / (6 as u16)) as u8;
    }

    let num_steps: u16 = marker_length
        - (3 as u16)
        - (1 as u16)
        - (1 as u16) * (image_num_components / (256 as u16));
    // Note: The conditions in the loop assume that we are reversibe
    for i in 0..((num_steps) as usize) {
        let step_size: u8 = data[pos + (i as usize)];
        if step_size & 0x07 != 0 {
            return (
                false,
                qcc_num_decomposition_levls,
                qcc_quantization_style,
                local_pos,
            );
        }
    }
    local_pos += num_steps as usize;
    (
        true,
        qcc_num_decomposition_levls,
        qcc_quantization_style,
        local_pos,
    )
}

pub fn parse_rgn(
    data: &[u8],
    pos: usize,
    marker_length: u16,
    image_num_components: u16,
) -> (bool, usize) {
    let mut local_pos: usize = pos;
    if marker_length < 5 || marker_length > 6 {
        return (false, local_pos);
    }
    if image_num_components < 257 {
        local_pos += 1;
    }
    local_pos += 1; // If num of components > 256, the index is 2 bytes, otherwise it is 1 byte.
    let srgn: u8 = data[local_pos];
    if srgn != 0 {
        return (false, local_pos);
    }
    local_pos += 2; // Skip the SPrgn byte.
    (true, local_pos)
}

pub fn parse_cod(data: &[u8], pos: usize, marker_length: u16) -> (bool, u8, usize) {
    let cod_num_decomposition_levls: u8;
    let scod: u8 = data[pos];
    let mut local_pos: usize = pos;
    if scod >> 3 != 0
    // Only first 3 bits might be non-zero
    {
        return (false, 0 as u8, local_pos);
    }

    // Parsing SGcod - includes Progression order, number of layers, Multiple Component Transformation
    let prog_order: u8 = data[local_pos + 1];
    if prog_order > 4 {
        return (false, 0 as u8, local_pos);
    }
    let num_layers: u16 =
        u16::from_be_bytes(data[local_pos + 2..local_pos + 4].try_into().unwrap());
    if num_layers <= 0 {
        return (false, 0 as u8, local_pos);
    }
    let multiple_component_transition: u8 = data[local_pos + 4];
    if multiple_component_transition > 1 {
        return (false, 0 as u8, local_pos);
    }

    // Parsing SPcod - Number of decomposition levels, code block width and height, code block style, Transformation, precinct size
    cod_num_decomposition_levls = data[local_pos + 5];
    if cod_num_decomposition_levls > 32 {
        return (false, cod_num_decomposition_levls, local_pos);
    }

    // Verift correct length
    if marker_length < 12 && marker_length > 45 {
        return (false, cod_num_decomposition_levls, local_pos);
    }
    if scod & 0x01 == 0 {
        if marker_length != 12 {
            return (false, cod_num_decomposition_levls, local_pos);
        }
    } else {
        if marker_length != (13 as u16) + (cod_num_decomposition_levls as u16) {
            return (false, cod_num_decomposition_levls, local_pos);
        }
    }

    let code_block_width: u8 = data[local_pos + 6];
    let code_block_height: u8 = data[local_pos + 7];
    if code_block_width + code_block_height > 8 {
        return (false, cod_num_decomposition_levls, local_pos);
    }
    let cod_block_style: u8 = data[local_pos + 8];
    if cod_block_style >> 5 != 0 {
        return (false, cod_num_decomposition_levls, local_pos);
    }
    let transformation: u8 = data[local_pos + 9];
    if transformation > 1 {
        return (false, cod_num_decomposition_levls, local_pos);
    }
    local_pos += 10;
    if scod & 0x01 == 1 {
        for i in 0..((cod_num_decomposition_levls + 1) as usize) {
            let precinct_size: u8 = data[local_pos + (i as usize)];
            let lsb_precinct_size: u8 = precinct_size & 0x0F;
            let msb_precinct_size: u8 = precinct_size >> 4;
            if i == 0 {
                if lsb_precinct_size > 15 || msb_precinct_size > 15 {
                    return (false, cod_num_decomposition_levls, local_pos);
                }
            } else {
                if lsb_precinct_size < 1 || lsb_precinct_size > 15 {
                    return (false, cod_num_decomposition_levls, local_pos);
                }
                if msb_precinct_size < 1 || msb_precinct_size > 15 {
                    return (false, cod_num_decomposition_levls, local_pos);
                }
            }
        }
        local_pos += (cod_num_decomposition_levls as usize) + 1;
    }
    (true, cod_num_decomposition_levls, local_pos)
}

pub fn parse_coc(
    data: &[u8],
    pos: usize,
    marker_length: u16,
    image_num_components: u16,
) -> (bool, u8, usize) {
    let mut local_pos: usize = pos;
    let coc_num_decomposition_levls: u8;
    if marker_length < 9 || marker_length > 43 {
        return (false, 0 as u8, local_pos);
    }
    if image_num_components < 257 {
        local_pos += 1;
    }
    local_pos += 1; // If num of components > 256, the index is 2 bytes, otherwise it is 1 byte.
    let scod: u8 = data[local_pos];
    if scod > 1 {
        return (false, 0 as u8, local_pos);
    }
    coc_num_decomposition_levls = data[local_pos + 1];
    if coc_num_decomposition_levls > 32 {
        return (false, coc_num_decomposition_levls, local_pos);
    }
    if marker_length
        != (8 as u16)
            + (scod as u16) * ((coc_num_decomposition_levls + 1) as u16)
            + (image_num_components / 2 as u16)
    {
        return (false, coc_num_decomposition_levls, local_pos);
    }
    let code_block_width: u8 = data[local_pos + 2];
    let code_block_height: u8 = data[local_pos + 3];
    if code_block_width + code_block_height > 8 {
        return (false, coc_num_decomposition_levls, local_pos);
    }
    let cod_block_style: u8 = data[local_pos + 4];
    if cod_block_style >> 5 != 0 {
        return (false, coc_num_decomposition_levls, local_pos);
    }
    let transformation: u8 = data[local_pos + 5];
    if transformation > 1 {
        return (false, coc_num_decomposition_levls, local_pos);
    }
    local_pos += 6;
    if scod == 1 {
        for i in 0..((coc_num_decomposition_levls + 1) as usize) {
            let precinct_size: u8 = data[local_pos + (i as usize)];
            let lsb_precinct_size: u8 = precinct_size & 0x0F;
            let msb_precinct_size: u8 = precinct_size >> 4;
            if i == 0 {
                if lsb_precinct_size > 15 || msb_precinct_size > 15 {
                    return (false, coc_num_decomposition_levls, local_pos);
                }
            } else {
                if lsb_precinct_size < 1 || lsb_precinct_size > 15 {
                    return (false, coc_num_decomposition_levls, local_pos);
                }
                if msb_precinct_size < 1 || msb_precinct_size > 15 {
                    return (false, coc_num_decomposition_levls, local_pos);
                }
            }
        }
        local_pos += (coc_num_decomposition_levls as usize) + 1;
    }
    (true, coc_num_decomposition_levls, local_pos)
}

pub fn parse_qcd(data: &[u8], pos: usize, marker_length: u16) -> (bool, u8, u8, usize) {
    let mut local_pos: usize = pos;
    let mut qcd_num_decomposition_levls: u8 = 0;
    if marker_length < 4 || marker_length > 197 {
        // println!("hello2"); // NO SUPPORTED JOLT
        return (false, qcd_num_decomposition_levls, 0 as u8, local_pos);
    }
    let qcd_quantization_style: u8 = data[local_pos];
    local_pos += 1;
    if qcd_quantization_style & 0x03 == 3 {
        // println!("hello3"); // NO SUPPORTED JOLT
        return (
            false,
            qcd_num_decomposition_levls,
            qcd_quantization_style,
            local_pos,
        );
    }
    // We are interested in reversible transorm only
    if qcd_quantization_style & 0x03 != 0 || qcd_quantization_style & 0x0E != 0 {
        // println!("hello4"); // NO SUPPORTED JOLT
        return (
            false,
            qcd_num_decomposition_levls,
            qcd_quantization_style,
            local_pos,
        );
    }

    // The ifdef also containes cases where the transform is not reversible, for clarity
    if qcd_quantization_style & 0x03 == 0 {
        // No quantization
        qcd_num_decomposition_levls = ((marker_length - 4 as u16) / (3 as u16)) as u8;
    } else if qcd_quantization_style & 0x03 == 1 {
        // Scalar quantization derived
        if marker_length != 5 {
            return (
                false,
                qcd_num_decomposition_levls,
                qcd_quantization_style,
                local_pos,
            );
        }
        qcd_num_decomposition_levls = 0;
    } else if qcd_quantization_style & 0x03 == 2 {
        // Scalar quantization expounded
        qcd_num_decomposition_levls = ((marker_length - 5 as u16) / (6 as u16)) as u8;
    }

    let num_step_sizes: u16 = marker_length - (3 as u16);
    // Note: The conditions in the loop assume that we are reversibe
    for i in 0..((num_step_sizes) as usize) {
        let step_size: u8 = data[pos + (i as usize)];
        if step_size & 0x07 != 0 {
            return (
                false,
                qcd_num_decomposition_levls,
                qcd_quantization_style,
                local_pos,
            );
        }
    }

    local_pos += num_step_sizes as usize;
    (
        true,
        qcd_num_decomposition_levls,
        qcd_quantization_style,
        local_pos,
    )
}
