import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getSettings, saveSettings } from "@/lib/settings";

export const settingsQueryKey = ["settings"] as const;

export function useSettings() {
  const queryClient = useQueryClient();

  const settings = useQuery({
    queryKey: settingsQueryKey,
    queryFn: getSettings,
  });

  const save = useMutation({
    mutationFn: saveSettings,
    onSuccess: (_data, newSettings) => {
      queryClient.setQueryData(settingsQueryKey, newSettings);
    },
  });

  return { settings, save };
}
