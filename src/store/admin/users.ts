import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '../../api/client'
import { User } from '../../types/api/user'

interface AdminUsersState {
  // Data
  users: User[]

  // User registration settings
  userRegistrationEnabled: boolean
  loadingRegistrationSettings: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Error state
  error: string | null
}

export const useAdminUsersStore = create<AdminUsersState>()(
  subscribeWithSelector(
    (): AdminUsersState => ({
      // Initial state
      users: [],
      userRegistrationEnabled: true,
      loadingRegistrationSettings: false,
      loading: false,
      creating: false,
      updating: false,
      deleting: false,
      error: null,
    }),
  ),
)

// User actions
export const loadAllSystemUsers = async (): Promise<void> => {
  try {
    useAdminUsersStore.setState({ loading: true, error: null })

    const response = await ApiClient.Admin.listUsers({
      page: 1,
      per_page: 50,
    })

    useAdminUsersStore.setState({
      users: response.users,
      loading: false,
    })
  } catch (error) {
    useAdminUsersStore.setState({
      error: error instanceof Error ? error.message : 'Failed to load users',
      loading: false,
    })
    throw error
  }
}

export const updateSystemUser = async (
  id: string,
  data: Partial<User>,
): Promise<User> => {
  try {
    useAdminUsersStore.setState({ updating: true, error: null })

    const user = await ApiClient.Admin.updateUser({ user_id: id, ...data })

    useAdminUsersStore.setState(state => ({
      users: state.users.map(u => (u.id === id ? user : u)),
      updating: false,
    }))

    return user
  } catch (error) {
    useAdminUsersStore.setState({
      error: error instanceof Error ? error.message : 'Failed to update user',
      updating: false,
    })
    throw error
  }
}

export const resetSystemUserPassword = async (
  id: string,
  newPassword: string,
): Promise<void> => {
  try {
    useAdminUsersStore.setState({ updating: true, error: null })

    await ApiClient.Admin.resetPassword({
      user_id: id,
      new_password: newPassword,
    })

    useAdminUsersStore.setState({ updating: false })
  } catch (error) {
    useAdminUsersStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to reset password',
      updating: false,
    })
    throw error
  }
}

export const toggleSystemUserActiveStatus = async (
  id: string,
): Promise<void> => {
  try {
    useAdminUsersStore.setState({ updating: true, error: null })

    await ApiClient.Admin.toggleUserActive({ user_id: id })

    useAdminUsersStore.setState(state => ({
      users: state.users.map(u =>
        u.id === id ? { ...u, is_active: !u.is_active } : u,
      ),
      updating: false,
    }))
  } catch (error) {
    useAdminUsersStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to toggle user status',
      updating: false,
    })
    throw error
  }
}

export const clearAdminUsersStoreError = (): void => {
  useAdminUsersStore.setState({ error: null })
}

// Registration settings
export const loadSystemUserRegistrationSettings = async (): Promise<void> => {
  try {
    useAdminUsersStore.setState({
      loadingRegistrationSettings: true,
      error: null,
    })

    const { enabled } = await ApiClient.Admin.getUserRegistrationStatus()

    useAdminUsersStore.setState({
      userRegistrationEnabled: enabled,
      loadingRegistrationSettings: false,
    })
  } catch (error) {
    useAdminUsersStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to load registration settings',
      loadingRegistrationSettings: false,
    })
    throw error
  }
}

export const updateSystemUserRegistrationSettings = async (
  enabled: boolean,
): Promise<void> => {
  try {
    useAdminUsersStore.setState({ updating: true, error: null })

    await ApiClient.Admin.updateUserRegistrationStatus({ enabled })

    useAdminUsersStore.setState({
      userRegistrationEnabled: enabled,
      updating: false,
    })
  } catch (error) {
    useAdminUsersStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to update registration settings',
      updating: false,
    })
    throw error
  }
}
