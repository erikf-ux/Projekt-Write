/** Project Write mark: a "W" drawn as a path through three mesh nodes. Decorative. */
export function Logo({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 1024 1024" aria-hidden="true" focusable="false">
      <rect x="32" y="32" width="960" height="960" rx="200" fill="var(--pw-color-sidebar)" />
      <polyline
        points="232,340 372,700 512,440 652,700 792,340"
        fill="none"
        stroke="var(--pw-color-accent)"
        strokeWidth="84"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="232" cy="340" r="56" fill="var(--pw-color-cluster-green)" />
      <circle cx="512" cy="440" r="56" fill="var(--pw-color-cluster-violet)" />
      <circle cx="792" cy="340" r="56" fill="var(--pw-color-cluster-orange)" />
    </svg>
  );
}
