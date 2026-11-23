import React, { useEffect, useMemo, useRef, useState } from "react";
import { formatBytes, formatRate } from "../lib/format";
import type { TimeSeriesPoint } from "../lib/types";

interface TrafficChartProps {
  points: TimeSeriesPoint[];
  compact?: boolean;
}

function buildSmoothPath(coords: Array<{ x: number; y: number }>): string {
  if (coords.length === 0) return "";
  if (coords.length === 1) return `M ${coords[0].x} ${coords[0].y}`;
  let d = `M ${coords[0].x.toFixed(1)} ${coords[0].y.toFixed(1)}`;
  for (let i = 0; i < coords.length - 1; i++) {
    const p0 = coords[i];
    const p1 = coords[i + 1];
    const cx = ((p0.x + p1.x) / 2).toFixed(1);
    d += ` C ${cx} ${p0.y.toFixed(1)}, ${cx} ${p1.y.toFixed(1)}, ${p1.x.toFixed(1)} ${p1.y.toFixed(1)}`;
  }
  return d;
}

export const TrafficChart: React.FC<TrafficChartProps> = ({ points, compact = false }) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [size, setSize] = useState({ width: compact ? 330 : 960, height: compact ? 84 : 185 });
  const [hoverIdx, setHoverIdx] = useState<number | null>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const w = Math.max(200, Math.round(entry.contentRect.width));
        const h = compact ? 84 : Math.max(140, Math.round(entry.contentRect.height));
        setSize((prev) => (prev.width === w && prev.height === h ? prev : { width: w, height: h }));
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [compact]);

  const { width, height } = size;
  const padLeft = compact ? 4 : 56;
  const padRight = compact ? 4 : 14;
  const padTop = compact ? 6 : 12;
  const padBottom = compact ? 6 : 24;
  const plotW = Math.max(40, width - padLeft - padRight);
  const plotH = Math.max(40, height - padTop - padBottom);

  const { rxCoords, txCoords, maxVal, yTicks, xTickIndices } = useMemo(() => {
    const safePoints =
      points.length > 1
        ? points
        : [
            { timestamp: 0, label: "", rxBytes: 0, txBytes: 0, peakBps: 0 },
            { timestamp: 1, label: "", rxBytes: 0, txBytes: 0, peakBps: 0 },
          ];
    let maxV = 1024;
    for (const p of safePoints) {
      if (p.rxBytes > maxV) maxV = p.rxBytes;
      if (p.txBytes > maxV) maxV = p.txBytes;
    }
    maxV = maxV * 1.15;

    const toCoord = (idx: number, val: number) => ({
      x: padLeft + (idx / Math.max(1, safePoints.length - 1)) * plotW,
      y: padTop + plotH - (val / maxV) * plotH,
    });

    const rx = safePoints.map((p, i) => toCoord(i, p.rxBytes));
    const tx = safePoints.map((p, i) => toCoord(i, p.txBytes));

    const ticks = [0, 0.33, 0.66, 1].map((ratio) => ({
      y: padTop + plotH - ratio * plotH,
      value: maxV * ratio,
    }));

    const xCount = Math.min(width < 600 ? 4 : 7, safePoints.length);
    const xIdx: number[] = [];
    if (safePoints.length > 1) {
      for (let k = 0; k < xCount; k++) {
        xIdx.push(Math.round((k / Math.max(1, xCount - 1)) * (safePoints.length - 1)));
      }
    }

    return { rxCoords: rx, txCoords: tx, maxVal: maxV, yTicks: ticks, xTickIndices: xIdx };
  }, [points, padLeft, padTop, plotW, plotH, width]);

  const rxLine = buildSmoothPath(rxCoords);
  const txLine = buildSmoothPath(txCoords);
  const baseY = (padTop + plotH).toFixed(1);

  const rxArea =
    rxCoords.length > 0
      ? `${rxLine} L ${rxCoords[rxCoords.length - 1].x.toFixed(1)} ${baseY} L ${rxCoords[0].x.toFixed(1)} ${baseY} Z`
      : "";
  const txArea =
    txCoords.length > 0
      ? `${txLine} L ${txCoords[txCoords.length - 1].x.toFixed(1)} ${baseY} L ${txCoords[0].x.toFixed(1)} ${baseY} Z`
      : "";

  const activePoint = hoverIdx !== null && points[hoverIdx] ? points[hoverIdx] : null;
  const activeCoord = hoverIdx !== null && rxCoords[hoverIdx] ? rxCoords[hoverIdx] : null;

  const handleMouseMove = (e: React.MouseEvent<SVGSVGElement>) => {
    if (compact || points.length === 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const relX = e.clientX - rect.left;
    const ratio = Math.max(0, Math.min(1, (relX - padLeft) / plotW));
    const idx = Math.round(ratio * (points.length - 1));
    setHoverIdx(idx);
  };

  return (
    <div
      ref={containerRef}
      className={`relative w-full select-none ${compact ? "h-[84px]" : "h-[168px] xl:h-[192px]"}`}
    >
      <svg
        width={width}
        height={height}
        viewBox={`0 0 ${width} ${height}`}
        className="block w-full h-full overflow-visible"
        onMouseMove={handleMouseMove}
        onMouseLeave={() => setHoverIdx(null)}
      >
        <defs>
          <linearGradient id="rxThemeFill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" style={{ stopColor: "var(--x-rx)", stopOpacity: 0.28 }} />
            <stop offset="100%" style={{ stopColor: "var(--x-rx)", stopOpacity: 0.01 }} />
          </linearGradient>
          <linearGradient id="txThemeFill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" style={{ stopColor: "var(--x-tx)", stopOpacity: 0.22 }} />
            <stop offset="100%" style={{ stopColor: "var(--x-tx)", stopOpacity: 0.01 }} />
          </linearGradient>
        </defs>

        {yTicks.map((t, i) => (
          <g key={i}>
            <line
              x1={padLeft}
              x2={width - padRight}
              y1={t.y}
              y2={t.y}
              stroke="rgba(255,255,255,0.065)"
              strokeDasharray={i === 0 ? undefined : "2 4"}
              strokeWidth={1}
            />
            {!compact && (
              <text
                x={padLeft - 10}
                y={t.y + 3.5}
                textAnchor="end"
                fill="rgba(161,161,170,0.72)"
                fontSize="10"
                className="font-mono tabular-nums"
              >
                {t.value <= 1 ? "0" : formatBytes(t.value)}
              </text>
            )}
          </g>
        ))}

        {!compact &&
          xTickIndices.map((idx) => {
            const pt = points[idx];
            const coord = rxCoords[idx];
            if (!pt || !coord) return null;
            return (
              <text
                key={idx}
                x={coord.x}
                y={height - 5}
                textAnchor="middle"
                fill="rgba(161,161,170,0.68)"
                fontSize="10"
                className="font-mono tabular-nums"
              >
                {pt.label}
              </text>
            );
          })}

        {rxArea && <path d={rxArea} fill="url(#rxThemeFill)" />}
        {txArea && <path d={txArea} fill="url(#txThemeFill)" />}

        {rxLine && (
          <path
            d={rxLine}
            fill="none"
            style={{ stroke: "var(--x-rx)" }}
            strokeWidth={compact ? 1.7 : 2}
            strokeLinejoin="round"
            strokeLinecap="round"
          />
        )}
        {txLine && (
          <path
            d={txLine}
            fill="none"
            style={{ stroke: "var(--x-tx)" }}
            strokeWidth={compact ? 1.3 : 1.6}
            strokeLinejoin="round"
            strokeLinecap="round"
          />
        )}

        {!compact && activeCoord && activePoint && (
          <g>
            <line
              x1={activeCoord.x}
              x2={activeCoord.x}
              y1={padTop}
              y2={padTop + plotH}
              stroke="rgba(255,255,255,0.22)"
              strokeWidth={1}
            />
            <circle
              cx={activeCoord.x}
              cy={activeCoord.y}
              r={4}
              style={{ fill: "var(--x-rx)" }}
              stroke="#000000"
              strokeWidth={2}
            />
          </g>
        )}
      </svg>

      {!compact && activePoint && activeCoord && maxVal > 0 && (
        <div
          className="pointer-events-none absolute top-1 z-20 rounded-xl px-3 py-1.5 text-[11px] font-mono tabular-nums text-white lg-pill whitespace-nowrap"
          style={{
            left: `clamp(130px, ${activeCoord.x}px, calc(100% - 140px))`,
            transform: "translateX(-50%)",
          }}
        >
          <span className="text-zinc-400 mr-2">{activePoint.label}</span>
          <span className="mr-2" style={{ color: "var(--x-rx)" }}>
            ↓ {formatBytes(activePoint.rxBytes)}
          </span>
          <span className="mr-2" style={{ color: "var(--x-tx)" }}>
            ↑ {formatBytes(activePoint.txBytes)}
          </span>
          <span className="text-zinc-400">Peak {formatRate(activePoint.peakBps)}</span>
        </div>
      )}
    </div>
  );
};
