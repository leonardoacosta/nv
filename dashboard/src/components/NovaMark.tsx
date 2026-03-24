interface NovaMarkProps {
  size?: number;
  className?: string;
}

export default function NovaMark({ size = 20, className = "" }: NovaMarkProps) {
  return (
    <svg
      viewBox="0 0 64 64"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      className={className}
      aria-label="Nova mark"
      role="img"
    >
      <defs>
        <linearGradient
          id="nm-sweep"
          x1="29"
          y1="30"
          x2="52"
          y2="11"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0%" stopColor="#c4b5fd" stopOpacity="0.55" />
          <stop offset="100%" stopColor="#c4b5fd" stopOpacity="0" />
        </linearGradient>
        <radialGradient
          id="nm-core-glow"
          cx="29"
          cy="30"
          r="8"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0%" stopColor="#7c3aed" stopOpacity="0.08" />
          <stop offset="100%" stopColor="#7c3aed" stopOpacity="0" />
        </radialGradient>
      </defs>
      {/* Faint partial arc */}
      <path
        d="M 10 44 A 25 25 0 0 1 54 24"
        stroke="#7c3aed"
        strokeWidth="0.35"
        opacity="0.07"
        fill="none"
      />
      {/* Center glow */}
      <circle cx="29" cy="30" r="10" fill="url(#nm-core-glow)" />
      {/* Center node */}
      <circle cx="29" cy="30" r="4.2" fill="#c4b5fd" />
      {/* Bright nearby stars */}
      <circle cx="22" cy="8" r="2.8" fill="#c4b5fd" opacity="0.8" />
      <circle cx="16" cy="50" r="2.5" fill="#c4b5fd" opacity="0.65" />
      {/* Medium stars */}
      <circle cx="52" cy="26" r="2" fill="#c4b5fd" opacity="0.5" />
      <circle cx="10" cy="28" r="1.6" fill="#c4b5fd" opacity="0.35" />
      <circle cx="46" cy="48" r="1.6" fill="#c4b5fd" opacity="0.38" />
      {/* Distant dust */}
      <circle cx="42" cy="10" r="0.9" fill="#c4b5fd" opacity="0.15" />
      <circle cx="36" cy="56" r="1" fill="#c4b5fd" opacity="0.12" />
      <circle cx="56" cy="42" r="0.7" fill="#c4b5fd" opacity="0.1" />
      {/* Curved organic connections */}
      <path
        d="M 29 30 Q 24 16, 22 8"
        stroke="#7c3aed"
        strokeWidth="0.55"
        opacity="0.2"
        fill="none"
      />
      <path
        d="M 29 30 Q 20 42, 16 50"
        stroke="#7c3aed"
        strokeWidth="0.55"
        opacity="0.2"
        fill="none"
      />
      <path
        d="M 29 30 Q 42 28, 52 26"
        stroke="#7c3aed"
        strokeWidth="0.45"
        opacity="0.15"
        fill="none"
      />
      {/* Faint cross-connections */}
      <path
        d="M 22 8 Q 14 16, 10 28"
        stroke="#7c3aed"
        strokeWidth="0.3"
        opacity="0.06"
        fill="none"
      />
      <path
        d="M 52 26 Q 50 38, 46 48"
        stroke="#7c3aed"
        strokeWidth="0.25"
        opacity="0.05"
        fill="none"
      />
      {/* Sweep with gradient */}
      <line
        x1="29"
        y1="29"
        x2="52"
        y2="11"
        stroke="url(#nm-sweep)"
        strokeWidth="1.4"
      />
      {/* Alert dot with halo */}
      <circle cx="52" cy="11" r="4" fill="#e11d48" opacity="0.06" />
      <circle cx="52" cy="11" r="2" fill="#e11d48" opacity="0.8" />
    </svg>
  );
}
