const BLOCK_BYTES: usize = 64;
const WORD_BYTES: usize = 4;
const LENGTH_OFFSET: usize = 56;
const PADDING_START: u8 = 0x80;
const BITS_PER_BYTE: u64 = 8;
const SCHEDULE_WORDS: usize = 64;
const HIGH_NIBBLE_SHIFT: u8 = 4;
const LOW_NIBBLE: u8 = 0x0f;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const INITIAL_STATE: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];
const ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
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
    message.extend_from_slice(&bit_length.to_be_bytes());
    let state = message
        .as_chunks::<BLOCK_BYTES>()
        .0
        .iter()
        .fold(INITIAL_STATE, compress);
    let mut digest = String::new();
    for byte in state.iter().flat_map(|word| word.to_be_bytes()) {
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

fn schedule(block: &[u8; BLOCK_BYTES]) -> Vec<u32> {
    let mut words: Vec<u32> = block
        .as_chunks::<WORD_BYTES>()
        .0
        .iter()
        .map(|chunk| u32::from_be_bytes(*chunk))
        .collect();
    while words.len() < SCHEDULE_WORDS {
        let at = |back: usize| words.get(words.len() - back).copied().unwrap_or_default();
        let early = at(15);
        let late = at(2);
        let small_early = early.rotate_right(7) ^ early.rotate_right(18) ^ (early >> 3);
        let small_late = late.rotate_right(17) ^ late.rotate_right(19) ^ (late >> 10);
        let next = at(16)
            .wrapping_add(small_early)
            .wrapping_add(at(7))
            .wrapping_add(small_late);
        words.push(next);
    }
    words
}

fn compress(state: [u32; 8], block: &[u8; BLOCK_BYTES]) -> [u32; 8] {
    let words = schedule(block);
    let mut working = state;
    for (constant, word) in ROUND_CONSTANTS.iter().zip(&words) {
        let [
            state_a,
            state_b,
            state_c,
            state_d,
            state_e,
            state_f,
            state_g,
            state_h,
        ] = working;
        let big_e = state_e.rotate_right(6) ^ state_e.rotate_right(11) ^ state_e.rotate_right(25);
        let choice = (state_e & state_f) ^ (!state_e & state_g);
        let first = state_h
            .wrapping_add(big_e)
            .wrapping_add(choice)
            .wrapping_add(*constant)
            .wrapping_add(*word);
        let big_a = state_a.rotate_right(2) ^ state_a.rotate_right(13) ^ state_a.rotate_right(22);
        let majority = (state_a & state_b) ^ (state_a & state_c) ^ (state_b & state_c);
        let second = big_a.wrapping_add(majority);
        working = [
            first.wrapping_add(second),
            state_a,
            state_b,
            state_c,
            state_d.wrapping_add(first),
            state_e,
            state_f,
            state_g,
        ];
    }
    let mut result = state;
    for (value, added) in result.iter_mut().zip(working) {
        *value = value.wrapping_add(added);
    }
    result
}
