export interface RuntimeFeatures {
  conversation: boolean;
  manualCompact: boolean;
  subagents: boolean;
  cron: boolean;
  remoteControl: boolean;
  tokenUsage: boolean;
}

/** Only the current runtime's observed profile can enable a feature. */
export function runtimeFeatures(session?: { runtimeFeatures?: Partial<RuntimeFeatures> }): RuntimeFeatures {
  const profile = session?.runtimeFeatures;
  const conversation = profile?.conversation === true;
  return {
    conversation,
    manualCompact: conversation && profile?.manualCompact === true,
    subagents: profile?.subagents === true,
    cron: profile?.cron === true,
    remoteControl: profile?.remoteControl === true,
    tokenUsage: profile?.tokenUsage === true,
  };
}
