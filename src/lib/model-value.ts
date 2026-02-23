import type { ModelProfile } from "./types";

export function profileToModelValue(profile: Pick<ModelProfile, "provider" | "model">): string {
  const provider = profile.provider.trim();
  const model = profile.model.trim();
  if (!provider) return model;
  if (!model) return `${provider}/`;
  const normalizedPrefix = `${provider.toLowerCase()}/`;
  if (model.toLowerCase().startsWith(normalizedPrefix)) {
    return model;
  }
  return `${provider}/${model}`;
}
