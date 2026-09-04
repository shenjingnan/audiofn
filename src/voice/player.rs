/// 音频播放（TTS 输出到扬声器）。
///
/// `AudioPlayer` trait 抽象播放能力，`VoiceSession` 依赖它而非具体实现：
/// 生产用 [`Speaker`]（rodio `Sink` 封装），测试注入 [`MockPlayer`]。
///
/// 句级流式的关键语义：
/// - `play` 把一段 PCM **追加**到播放队列（`Sink::append` 非阻塞，按序播放）；
/// - `stop` **drop 当前 Sink**（rodio 的 `Sink::stop()` 后 `append` 不再播放，内部
///   stopped 置位，因此打断用「drop 重建」，下一次 `play` 时 `ensure_sink` 重建）；
/// - `drained` 判断当前 + 排队全部播完（对应「播完 → 回听」）。
pub trait AudioPlayer {
    /// 追加一段 f32 PCM（mono）到播放队列，非阻塞。
    fn play(&mut self, samples: Vec<f32>, sample_rate: u32);
    /// 立即停止并清空播放队列（打断）。
    fn stop(&mut self);
    /// 当前及排队音频是否已全部播完。
    fn drained(&self) -> bool;
}

/// rodio 0.22 播放器。
///
/// 持有 `MixerDeviceSink`（保持输出流存活）+ `Player`（控制播放队列）。
/// rodio 0.22 的 `Player::stop()` 只是置停止标志、下次 `append` 自动恢复播放，
/// 因此打断用 `stop()` 即可（无需 drop 重建）；`empty()` 判断播完。
pub struct Speaker {
    _sink: rodio::stream::MixerDeviceSink,
    player: rodio::Player,
}

impl Speaker {
    /// 打开默认输出设备。失败返回错误（无音频输出设备等）。
    pub fn try_new() -> Result<Self, String> {
        let builder = rodio::stream::DeviceSinkBuilder::from_default_device()
            .map_err(|e| format!("无法打开音频输出设备: {e}"))?;
        let mut sink = builder
            .open_stream()
            .map_err(|e| format!("无法打开音频输出流: {e}"))?;
        // 正常 Drop 时静音（避免每次退出打印 "Dropping DeviceSink" 噪音）
        sink.log_on_drop(false);
        let player = rodio::Player::connect_new(sink.mixer());
        Ok(Self {
            _sink: sink,
            player,
        })
    }
}

impl AudioPlayer for Speaker {
    fn play(&mut self, samples: Vec<f32>, sample_rate: u32) {
        let channels = rodio::ChannelCount::new(1).expect("1 > 0");
        let rate = rodio::SampleRate::new(sample_rate).expect("sample_rate > 0");
        // append 前若处于 stopped 状态会自动恢复播放（rodio 0.22 语义）
        self.player
            .append(rodio::buffer::SamplesBuffer::new(channels, rate, samples));
    }

    fn stop(&mut self) {
        self.player.stop();
    }

    fn drained(&self) -> bool {
        self.player.empty()
    }
}

/// 测试用播放器：记录调用序列，无真实设备依赖。
pub struct MockPlayer {
    pub plays: Vec<(Vec<f32>, u32)>,
    pub stops: usize,
    pub drained_value: bool,
}

impl MockPlayer {
    pub fn new() -> Self {
        Self {
            plays: Vec::new(),
            stops: 0,
            drained_value: true,
        }
    }
}

impl Default for MockPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioPlayer for MockPlayer {
    fn play(&mut self, samples: Vec<f32>, sample_rate: u32) {
        self.plays.push((samples, sample_rate));
    }

    fn stop(&mut self) {
        self.stops += 1;
        self.plays.clear();
    }

    fn drained(&self) -> bool {
        self.drained_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_player_records_plays() {
        let mut p = MockPlayer::new();
        assert!(p.drained());
        p.play(vec![0.1, 0.2, 0.3], 24000);
        p.play(vec![0.4, 0.5], 24000);
        assert_eq!(p.plays.len(), 2);
        assert_eq!(p.plays[0].1, 24000);
        assert_eq!(p.plays[1].0, vec![0.4, 0.5]);
    }

    #[test]
    fn test_mock_player_stop_clears() {
        let mut p = MockPlayer::new();
        p.play(vec![1.0], 24000);
        p.drained_value = false;
        assert!(!p.drained());
        p.stop();
        assert_eq!(p.stops, 1);
        assert!(p.plays.is_empty());
    }

    #[test]
    fn test_mock_player_drained_flag() {
        let mut p = MockPlayer::new();
        p.drained_value = false;
        assert!(!p.drained());
        p.drained_value = true;
        assert!(p.drained());
    }
}
