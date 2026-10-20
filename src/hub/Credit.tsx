import { INSPIRED_BY } from "../brand";

/** "Inspired by <original>" with the original's official logo, small and unmodified. */
export default function Credit({ compact }: { compact?: boolean }) {
  if (!INSPIRED_BY) return null;
  return (
    <div className={`credit ${compact ? "compact" : ""}`}>
      <span className="credit-line">
        Inspired by <img src={INSPIRED_BY.logo} alt={INSPIRED_BY.name} className="credit-logo" /> <span className="sr">{INSPIRED_BY.name}</span>
      </span>
      <span className="credit-note">Independent open-source project, not affiliated with or endorsed by {INSPIRED_BY.company}.</span>
    </div>
  );
}
