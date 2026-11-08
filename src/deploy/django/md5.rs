const BLOCK_BYTES: usize = 64;
const WORD_BYTES: usize = 4;
const LENGTH_OFFSET: usize = 56;
const PADDING_START: u8 = 0x80;
const BITS_PER_BYTE: u64 = 8;
const ROUNDS_PER_STAGE: usize = 16;
const SHIFTS_PER_STAGE: usize = 4;
const HIGH_NIBBLE_SHIFT: u8 = 4;
const LOW_NIBBLE: u8 = 0x0f;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const INITIAL_STATE: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
const SHIFTS: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];
const WORD_STEPS: [(usize, usize); 4] = [(1, 0), (5, 1), (3, 5), (7, 0)];
const SINES: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee,
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501,
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be,
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821,
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa,
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8,
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed,
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a,
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c,
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70,
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05,
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665,
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039,
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1,
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1,
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];

pub fn hex_digest(input: &[u8]) -> String {
    let bit_length = u64::try_from(input.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(BITS_PER_BYTE);
    let mut message = input.to_vec();
    message.push(PADDING_START);
    while message.len() % BLOCK_BYTES != LENGTH_OFFSET {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_le_bytes());
    let state = message
        .as_chunks::<BLOCK_BYTES>()
        .0
        .iter()
        .fold(INITIAL_STATE, compress);
    let mut digest = String::new();
    for byte in state.iter().flat_map(|word| word.to_le_bytes()) {
        digest.push(hex_digit(byte >> HIGH_NIBBLE_SHIFT));
        digest.push(hex_digit(byte & LOW_NIBBLE));
    }
    digest
}

fn hex_digit(nibble: u8) -> char {
    HEX_DIGITS
        .get(usize::from(nibble))
        .copied()
        .map_or('0', char::from)
}

fn compress(state: [u32; 4], block: &[u8; BLOCK_BYTES]) -> [u32; 4] {
    let words: Vec<u32> = block
        .as_chunks::<WORD_BYTES>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .collect();
    let [mut a, mut b, mut c, mut d] = state;
    for (round, sine) in SINES.iter().enumerate() {
        let round_group = round / ROUNDS_PER_STAGE;
        let mixed = mix(round_group, b, c, d);
        let (multiplier, offset) = WORD_STEPS.get(round_group).copied().unwrap_or_default();
        let word_index = (multiplier * round + offset) % ROUNDS_PER_STAGE;
        let word = words.get(word_index).copied().unwrap_or_default();
        let shift = SHIFTS
            .get(round_group * SHIFTS_PER_STAGE + round % SHIFTS_PER_STAGE)
            .copied()
            .unwrap_or_default();
        let rotated = a
            .wrapping_add(mixed)
            .wrapping_add(*sine)
            .wrapping_add(word)
            .rotate_left(shift);
        a = d;
        d = c;
        c = b;
        b = b.wrapping_add(rotated);
    }
    let [first, second, third, fourth] = state;
    [
        first.wrapping_add(a),
        second.wrapping_add(b),
        third.wrapping_add(c),
        fourth.wrapping_add(d),
    ]
}

const fn mix(stage: usize, b: u32, c: u32, d: u32) -> u32 {
    match stage {
        0 => (b & c) | (!b & d),
        1 => (d & b) | (!d & c),
        2 => b ^ c ^ d,
        _ => c ^ (b | !d),
    }
}
