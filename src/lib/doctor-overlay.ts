export function resolveDoctorOverlayVisibility(params: {
  showDoctorUi: boolean;
  guidanceOpen: boolean;
  hasGuidance: boolean;
}) {
  const showAssistantPill = false;
  const showGuidanceCard = showAssistantPill && params.guidanceOpen && params.hasGuidance;

  return {
    showQuickDiagnose: params.showDoctorUi,
    showGuidanceCard,
    showAssistantPill,
    showGuidanceOverlay: showGuidanceCard || showAssistantPill,
  };
}
