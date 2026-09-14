use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC};
use windows::core::w;

pub fn play_feedback(muted: bool) {
    unsafe {
        // SystemHand (Critical Stop / low tone) for Muted
        // SystemNotification / SystemAsterisk for Unmuted
        let sound_name = if muted {
            w!("SystemHand")
        } else {
            w!("SystemNotification")
        };
        let _ = PlaySoundW(sound_name, None, SND_ALIAS | SND_ASYNC);
    }
}
