import type { Snapshot } from "../types";

const SIZE = 194;
const STROKE = 7;
const RADIUS = (SIZE - STROKE) / 2 - 1;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export function Ring({ timer }: { timer: Snapshot }) {
  // The backend only speaks once a second. The stylesheet interpolates the
  // second in between, which is why this can be a plain number.
  const offset = CIRCUMFERENCE * (1 - Math.min(1, Math.max(0, timer.progress)));

  return (
    <div className="ring" data-state={timer.state}>
      <svg width={SIZE} height={SIZE} aria-hidden="true">
        <circle className="ring__track" cx={SIZE / 2} cy={SIZE / 2} r={RADIUS} />
        <circle
          className="ring__progress"
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={RADIUS}
          strokeDasharray={CIRCUMFERENCE}
          strokeDashoffset={offset}
        />
      </svg>
      <div className="ring__readout">
        <span className="ring__clock">{timer.clock}</span>
        <span className="ring__phase">{timer.phaseLabel}</span>
      </div>
    </div>
  );
}
