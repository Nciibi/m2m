
export function NoChatsIllustration() {
  return (
    <div className="relative w-48 h-48 flex items-center justify-center mb-xl select-none pointer-events-none">
      {/* Background soft glow */}
      <div className="absolute inset-0 bg-primary/5 rounded-full filter blur-xl animate-pulse"></div>
      
      {/* Grid Pattern */}
      <svg width="180" height="180" viewBox="0 0 180 180" fill="none" xmlns="http://www.w3.org/2000/svg" className="absolute opacity-20">
        <defs>
          <pattern id="grid" width="20" height="20" patternUnits="userSpaceOnUse">
            <path d="M 20 0 L 0 0 0 20" fill="none" stroke="var(--color-text-secondary)" strokeWidth="0.5" />
          </pattern>
        </defs>
        <rect width="180" height="180" fill="url(#grid)" />
      </svg>

      {/* Main geometric chat shapes */}
      <svg width="140" height="140" viewBox="0 0 140 140" fill="none" xmlns="http://www.w3.org/2000/svg" className="relative">
        {/* Connection line */}
        <path d="M40 70 H100" stroke="var(--color-accent)" strokeWidth="1.5" strokeDasharray="3 3" className="animate-[dash_10s_linear_infinite]" />
        
        {/* Left Bubble */}
        <g className="animate-[bounce_4s_infinite_alternate]">
          <rect x="25" y="45" width="40" height="30" rx="8" fill="var(--color-bg-card)" stroke="var(--color-border-default)" strokeWidth="1.5" />
          <path d="M25 65 L20 70 L25 72" fill="var(--color-bg-card)" stroke="var(--color-border-default)" strokeWidth="1.5" />
          <line x1="33" y1="55" x2="57" y2="55" stroke="var(--color-text-muted)" strokeWidth="1.5" strokeLinecap="round" />
          <line x1="33" y1="60" x2="48" y2="60" stroke="var(--color-accent)" strokeWidth="1.5" strokeLinecap="round" />
          <circle cx="33" cy="50" r="1.5" fill="var(--color-accent)" />
        </g>

        {/* Right Bubble */}
        <g className="animate-[bounce_4s_infinite_alternate]" style={{ animationDelay: "1s" }}>
          <rect x="75" y="65" width="40" height="30" rx="8" fill="var(--color-bg-surface)" stroke="var(--color-accent)" strokeWidth="1" />
          <path d="M115 85 L120 90 L115 92" fill="var(--color-bg-surface)" stroke="var(--color-accent)" strokeWidth="1" />
          <line x1="83" y1="75" x2="107" y2="75" stroke="var(--color-text-primary)" strokeWidth="1.5" strokeLinecap="round" />
          <line x1="83" y1="80" x2="98" y2="80" stroke="var(--color-text-muted)" strokeWidth="1.5" strokeLinecap="round" />
        </g>

        {/* Floating security lock symbol */}
        <g className="animate-[bounce_3s_infinite_alternate]" style={{ animationDelay: "0.5s" }}>
          <circle cx="70" cy="40" r="12" fill="var(--color-bg-elevated)" stroke="var(--color-border-default)" strokeWidth="1" />
          <path d="M67 42 H73 V46 H67 V42 Z" stroke="var(--color-accent)" strokeWidth="1" />
          <path d="M68 42 V40 C68 38.5 72 38.5 72 40 V42" stroke="var(--color-accent)" strokeWidth="1" fill="none" />
        </g>
      </svg>
    </div>
  );
}

export function RadarIllustration() {
  return (
    <div className="relative w-56 h-56 flex items-center justify-center mb-xl select-none pointer-events-none">
      {/* Background Soft Glow */}
      <div className="absolute inset-0 bg-primary/5 rounded-full filter blur-xl animate-pulse"></div>

      <svg width="220" height="220" viewBox="0 0 220 220" fill="none" xmlns="http://www.w3.org/2000/svg" className="absolute">
        {/* Outer dotted ring */}
        <circle cx="110" cy="110" r="95" stroke="var(--color-border-default)" strokeWidth="1" strokeDasharray="4 4" />
        
        {/* Middle ring */}
        <circle cx="110" cy="110" r="65" stroke="var(--color-border-default)" strokeWidth="1.2" />
        
        {/* Inner ring */}
        <circle cx="110" cy="110" r="35" stroke="var(--color-accent)" strokeWidth="0.8" opacity="0.3" />

        {/* Center Node */}
        <circle cx="110" cy="110" r="6" fill="var(--color-accent)" />
        <circle cx="110" cy="110" r="12" stroke="var(--color-accent)" strokeWidth="1.5" className="animate-ping" />

        {/* Sweeping Radar Line */}
        <line 
          x1="110" 
          y1="110" 
          x2="110" 
          y2="15" 
          stroke="var(--color-accent)" 
          strokeWidth="1.5" 
          className="origin-[110px_110px] animate-[spin_6s_linear_infinite]"
        />

        {/* Pulsing Target Node 1 */}
        <g className="animate-[pulse_2s_infinite]">
          <circle cx="170" cy="70" r="4" fill="var(--color-success)" />
          <circle cx="170" cy="70" r="10" stroke="var(--color-success)" strokeWidth="0.5" opacity="0.5" />
        </g>

        {/* Pulsing Target Node 2 */}
        <g className="animate-[pulse_3s_infinite]" style={{ animationDelay: "1s" }}>
          <circle cx="50" cy="130" r="3" fill="var(--color-success)" />
          <circle cx="50" cy="130" r="8" stroke="var(--color-success)" strokeWidth="0.5" opacity="0.5" />
        </g>
      </svg>
    </div>
  );
}

export function FamilyIllustration() {
  return (
    <div className="relative w-48 h-48 flex items-center justify-center mb-xl select-none pointer-events-none">
      <div className="absolute inset-0 bg-primary/5 rounded-full filter blur-xl animate-pulse"></div>

      <svg width="180" height="180" viewBox="0 0 180 180" fill="none" xmlns="http://www.w3.org/2000/svg">
        {/* Trust Node 1 (Center Top) */}
        <g className="animate-[bounce_5s_infinite_alternate]">
          <circle cx="90" cy="45" r="14" fill="var(--color-bg-card)" stroke="var(--color-accent)" strokeWidth="1.5" />
          <path d="M85 48 C85 43 95 43 95 48" stroke="var(--color-accent)" strokeWidth="1" fill="none" />
          <circle cx="90" cy="41" r="3.5" stroke="var(--color-accent)" strokeWidth="1" fill="none" />
        </g>

        {/* Trust Node 2 (Bottom Left) */}
        <g className="animate-[bounce_5s_infinite_alternate]" style={{ animationDelay: "0.8s" }}>
          <circle cx="45" cy="120" r="14" fill="var(--color-bg-card)" stroke="var(--color-border-default)" strokeWidth="1.5" />
          <path d="M40 123 C40 118 50 118 50 123" stroke="var(--color-text-muted)" strokeWidth="1" fill="none" />
          <circle cx="45" cy="116" r="3.5" stroke="var(--color-text-muted)" strokeWidth="1" fill="none" />
        </g>

        {/* Trust Node 3 (Bottom Right) */}
        <g className="animate-[bounce_5s_infinite_alternate]" style={{ animationDelay: "1.6s" }}>
          <circle cx="135" cy="120" r="14" fill="var(--color-bg-card)" stroke="var(--color-border-default)" strokeWidth="1.5" />
          <path d="M130 123 C130 118 140 118 140 123" stroke="var(--color-text-muted)" strokeWidth="1" fill="none" />
          <circle cx="135" cy="116" r="3.5" stroke="var(--color-text-muted)" strokeWidth="1" fill="none" />
        </g>

        {/* Trust connections */}
        <path d="M90 59 L45 106" stroke="var(--color-border-default)" strokeWidth="1.2" strokeDasharray="3 3" />
        <path d="M90 59 L135 106" stroke="var(--color-border-default)" strokeWidth="1.2" strokeDasharray="3 3" />
        <path d="M59 120 H121" stroke="var(--color-accent)" strokeWidth="1" opacity="0.4" strokeDasharray="2 2" />
        
        {/* Heart/Shield Center Shield */}
        <g className="animate-pulse">
          <circle cx="90" cy="95" r="10" fill="var(--color-bg-surface)" stroke="var(--color-accent)" strokeWidth="1" />
          <path d="M90 91.5 L93 93 V96.5 L90 98.5 L87 96.5 V93 L90 91.5 Z" fill="var(--color-accent)" opacity="0.8" />
        </g>
      </svg>
    </div>
  );
}
