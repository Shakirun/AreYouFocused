use rand::Rng;

pub fn next_interval_minutes<R: Rng + ?Sized>(min_m: i64, max_m: i64, rng: &mut R) -> i64 {
    assert!(min_m >= 1 && max_m >= min_m);
    rng.gen_range(min_m..=max_m)
}

pub fn next_ping_after<R: Rng + ?Sized>(now_unix: i64, min_m: i64, max_m: i64, rng: &mut R) -> i64 {
    let delta = next_interval_minutes(min_m, max_m, rng);
    now_unix.saturating_add(delta * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_interval_minutes_respects_bounds() {
        for _ in 0..200 {
            let m = next_interval_minutes(30, 120, &mut rand::thread_rng());
            assert!((30..=120).contains(&m));
        }
    }

    #[test]
    fn next_ping_unix_is_in_future() {
        let now = 1_700_000_000_i64;
        let next = next_ping_after(now, 30, 120, &mut rand::thread_rng());
        assert!(next > now);
        assert!(next <= now + 120 * 60 + 1);
        assert!(next >= now + 30 * 60);
    }
}
