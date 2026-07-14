import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { authLogin, authLogout, authStatus } from "@/lib/auth";

const authQueryKey = ["auth", "status"] as const;

export function useAuth() {
  const queryClient = useQueryClient();

  const status = useQuery({
    queryKey: authQueryKey,
    queryFn: authStatus,
  });

  const login = useMutation({
    mutationFn: authLogin,
    onSuccess: (user) => {
      queryClient.setQueryData(authQueryKey, user);
    },
  });

  const logout = useMutation({
    mutationFn: authLogout,
    onSuccess: () => {
      queryClient.setQueryData(authQueryKey, null);
    },
  });

  return { status, login, logout };
}
