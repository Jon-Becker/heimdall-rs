use rand::Rng;
use std::iter;

pub fn random_string(len: usize) -> String {
    const CHARSET: &[u8] = b"qwertyuiopasdfghjklzxcvbnm0123456789";
    let mut rng = rand::thread_rng();
    let one_char = || CHARSET[rng.gen_range(0..CHARSET.len())] as char;
    iter::repeat_with(one_char).take(len).collect()
}