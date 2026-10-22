export function LogoMark({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" aria-hidden>
      <rect x="64" y="64" width="896" height="896" rx="216" fill="var(--accent)" />
      <path d="M512 176C512 176 292 440 292 600a220 220 0 0 0 440 0c0-160-220-424-220-424Z" fill="var(--surface)" />
      <g fill="none" stroke="var(--accent)" strokeWidth="54" strokeLinecap="round" strokeLinejoin="round">
        <path d="M404 560h214M560 500l60 60-60 60" />
        <path d="M620 680H406M464 620l-60 60 60 60" />
      </g>
    </svg>
  );
}
