interface UseChromiumTermsReturn {
  termsAccepted: boolean | null;
  isLoading: boolean;
  checkTerms: () => Promise<void>;
}

// Chromium terms are no longer needed in the UI - always return accepted
export function useChromiumTerms(): UseChromiumTermsReturn {
  return {
    termsAccepted: true,
    isLoading: false,
    checkTerms: async () => {},
  };
}
