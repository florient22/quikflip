// Countdown to the next day, based on wall-clock time so it stays right even if the tab sleeps.

const TICK_MS = 1000;

export function formatDuration(totalSeconds) {
  const seconds = Math.max(0, Math.round(totalSeconds));
  return [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60].map(part => String(part).padStart(2, '0')).join(':');
}

/** Calls onTick(secondsLeft) every second and onDone() once at zero. Returns a stop function. */
export function startCountdown(secondsLeft, onTick, onDone) {
  const endsAt = Date.now() + secondsLeft * 1000;
  let finished = false;
  const tick = () => {
    const left = Math.max(0, (endsAt - Date.now()) / 1000);
    onTick(left);
    if (left <= 0 && !finished) {
      finished = true;
      clearInterval(timer);
      onDone();
    }
  };
  const timer = setInterval(tick, TICK_MS);
  tick();
  return () => clearInterval(timer);
}
