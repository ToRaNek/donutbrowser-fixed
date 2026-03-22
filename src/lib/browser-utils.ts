/**
 * Browser utility functions
 * Centralized helpers for browser name mapping, icons, etc.
 */

import { FaExclamationTriangle, FaFire, FaFirefox } from "react-icons/fa";

/**
 * Map internal browser names to display names
 * Note: "chromium" kept for backward compat with existing profiles
 */
export function getBrowserDisplayName(browserType: string): string {
  const browserNames: Record<string, string> = {
    camoufox: "Camoufox",
    chromium: "Camoufox", // Display existing chromium profiles as Camoufox in UI
  };

  return browserNames[browserType] || browserType;
}

/**
 * Get the appropriate icon component for a browser type
 * Anti-detect browsers get their base browser icons
 * Other browsers get a warning icon to indicate they're not anti-detect
 */
export function getBrowserIcon(browserType: string) {
  switch (browserType) {
    case "camoufox":
      return FaFirefox; // Firefox-based anti-detect browser
    case "chromium":
      return FaFirefox; // Display as Camoufox icon for existing chromium profiles
    default:
      // All other browsers get a warning icon
      return FaExclamationTriangle;
  }
}

export function getProfileIcon(profile: {
  browser: string;
  ephemeral?: boolean;
}) {
  if (profile.ephemeral) return FaFire;
  return getBrowserIcon(profile.browser);
}

export const getCurrentOS = () => {
  if (typeof window !== "undefined") {
    const userAgent = window.navigator.userAgent;
    if (userAgent.includes("Win")) return "windows";
    if (userAgent.includes("Mac")) return "macos";
    if (userAgent.includes("Linux")) return "linux";
  }
  return "unknown";
};

export function isCrossOsProfile(profile: {
  host_os?: string;
  camoufox_config?: { os?: string };
  chromium_config?: { os?: string };
}): boolean {
  const profileOs =
    profile.host_os ||
    profile.camoufox_config?.os ||
    profile.chromium_config?.os;
  if (!profileOs) return false;
  return profileOs !== getCurrentOS();
}

export function getOSDisplayName(os: string): string {
  switch (os) {
    case "macos":
      return "macOS";
    case "windows":
      return "Windows";
    case "linux":
      return "Linux";
    default:
      return os;
  }
}
