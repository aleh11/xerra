import React from "react";

/**
 * Xerra wordmark. Kept at this path (and export name) so upstream Handy
 * rebases don't conflict on every import; it renders a plain text wordmark
 * in the same logo colors until proper brand art exists.
 */
const HandyTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 360 110"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label="Xerra"
    >
      <text
        x="180"
        y="82"
        textAnchor="middle"
        fontFamily="system-ui, -apple-system, 'Segoe UI', sans-serif"
        fontSize="96"
        fontWeight="800"
        letterSpacing="-2"
        className="logo-stroke"
        strokeWidth="10"
        strokeLinejoin="round"
      >
        {"Xerra"}
      </text>
      <text
        x="180"
        y="82"
        textAnchor="middle"
        fontFamily="system-ui, -apple-system, 'Segoe UI', sans-serif"
        fontSize="96"
        fontWeight="800"
        letterSpacing="-2"
        className="logo-primary"
      >
        {"Xerra"}
      </text>
    </svg>
  );
};

export default HandyTextLogo;
