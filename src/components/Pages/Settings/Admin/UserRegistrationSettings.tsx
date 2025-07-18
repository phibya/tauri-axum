import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { App, Card, Form, Switch, Typography } from 'antd'
import { useShallow } from 'zustand/react/shallow'
import { Permission, usePermissions } from '../../../../permissions'
import { PageContainer } from '../../../common/PageContainer'
import { useAdminStore } from '../../../../store/admin'

const { Text } = Typography

export function UserRegistrationSettings() {
  const { t } = useTranslation()
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const { hasPermission } = usePermissions()

  // Admin store
  const {
    registrationEnabled,
    loading,
    error,
    loadUserRegistrationSettings,
    updateUserRegistrationSettings,
    clearError,
  } = useAdminStore(
    useShallow(state => ({
      registrationEnabled: state.userRegistrationEnabled,
      loading: state.loading,
      error: state.error,
      loadUserRegistrationSettings: state.loadUserRegistrationSettings,
      updateUserRegistrationSettings: state.updateUserRegistrationSettings,
      clearError: state.clearError,
    })),
  )

  const canRead = hasPermission(Permission.config.userRegistration.read)
  const canEdit = hasPermission(Permission.config.userRegistration.edit)

  useEffect(() => {
    if (canRead) {
      loadUserRegistrationSettings()
    }
  }, [canRead, loadUserRegistrationSettings])

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      clearError()
    }
  }, [error, message, clearError])

  // Update form when registration status changes
  useEffect(() => {
    form.setFieldsValue({ enabled: registrationEnabled })
  }, [registrationEnabled, form])

  const handleFormChange = async (changedValues: any) => {
    if (!canEdit) {
      message.error(t('admin.noPermissionEditSetting'))
      return
    }
    if ('enabled' in changedValues) {
      const newValue = changedValues.enabled

      try {
        await updateUserRegistrationSettings(newValue)
        message.success(
          `User registration ${newValue ? 'enabled' : 'disabled'} successfully`,
        )
      } catch (error) {
        console.error('Failed to update registration status:', error)
        // Error is handled by the store
      }
    }
  }

  if (!canRead) {
    return null
  }

  return (
    <PageContainer>
      <Card title={t('admin.userRegistration')} className="mb-6">
        <Form
          form={form}
          onValuesChange={handleFormChange}
          initialValues={{ enabled: registrationEnabled }}
        >
          <div className="flex justify-between items-center">
            <div>
              <Text strong>Enable User Registration</Text>
              <div>
                <Text type="secondary">
                  Allow new users to register for accounts
                </Text>
              </div>
            </div>
            <Form.Item name="enabled" valuePropName="checked" className="mb-0">
              <Switch loading={loading} size="default" />
            </Form.Item>
          </div>
        </Form>
      </Card>
    </PageContainer>
  )
}
