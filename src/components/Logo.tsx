import { useId } from "react";

/** EasyTizen mark: a screen receiving an app. Original artwork — not derived from any
 *  Samsung or Tizen logo. The same drawing is used for the app icon (assets/logo.svg). */

export function Logo({ size = 28 }: { size?: number }) {
  // Unique per instance: a gradient defined inside a hidden copy would not render.
  const id = useId();
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#3F5BF6" />
          <stop offset="1" stopColor="#12B8A6" />
        </linearGradient>
      </defs>
      <rect width="64" height="64" rx="15" fill={`url(#${id})`} />
      <rect x="12" y="16" width="40" height="26" rx="5" fill="none" stroke="#fff" strokeWidth="4" />
      <path d="M24 50h16" stroke="#fff" strokeWidth="4" strokeLinecap="round" />
      <path d="M32 21.5v12M26.5 28.5l5.5 5.5 5.5-5.5" fill="none" stroke="#fff" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
