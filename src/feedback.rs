use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY};
use windows::core::PCWSTR;

const MUTED_WAV: &[u8] = include_bytes!("../muted.wav");
const UNMUTED_WAV: &[u8] = include_bytes!("../unmuted.wav");

pub fn play_feedback(muted: bool) {
    unsafe {
        let sound_data = if muted { MUTED_WAV } else { UNMUTED_WAV };
        let _ = PlaySoundW(
            PCWSTR(sound_data.as_ptr() as *const u16),
            None,
            SND_MEMORY | SND_ASYNC,
        );
    }
}
