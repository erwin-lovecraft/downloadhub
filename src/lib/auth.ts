import { invoke } from "@tauri-apps/api/core";

export interface UserInfo {
  email: string;
  name: string | null;
  picture: string | null;
}

export const authStatus = () => invoke<UserInfo | null>("auth_status");
export const authLogin = () => invoke<UserInfo>("auth_login");
export const authLogout = () => invoke<void>("auth_logout");
