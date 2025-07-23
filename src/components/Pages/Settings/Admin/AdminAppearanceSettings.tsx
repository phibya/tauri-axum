import { App, Card, Flex, Form, Select, Space, Typography } from 'antd'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useShallow } from 'zustand/react/shallow'
import {
  useUserSettingsStore,
  loadGlobalDefaultLanguage,
} from '../../../../store'
import { useAdminStore, updateSystemDefaultLanguage } from '../../../../store'
import { isDesktopApp } from '../../../../api/core'
import { Permission, usePermissions } from '../../../../permissions'

const { Title, Text } = Typography

export function AdminAppearanceSettings() {
  const { t } = useTranslation()
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [isMobile, setIsMobile] = useState(false)
  const { hasPermission } = usePermissions()
  const { globalDefaultLanguage } = useUserSettingsStore()

  // Admin store
  const { updating } = useAdminStore(
    useShallow(state => ({
      updating: state.updating,
    })),
  )

  // Check permissions - using a general config permission for appearance settings
  const canEditAppearance = hasPermission(Permission.config.experimental.edit)

  useEffect(() => {
    const checkMobile = () => {
      setIsMobile(window.innerWidth < 768)
    }

    checkMobile()
    window.addEventListener('resize', checkMobile)

    return () => window.removeEventListener('resize', checkMobile)
  }, [])

  useEffect(() => {
    form.setFieldsValue({
      language: globalDefaultLanguage,
    })
  }, [globalDefaultLanguage, form])

  const handleFormChange = async (changedValues: any) => {
    if ('language' in changedValues) {
      if (!canEditAppearance) {
        message.error(t('admin.noPermissionSystemSettings'))
        form.setFieldsValue({ language: globalDefaultLanguage })
        return
      }

      try {
        // Update global default language via admin store
        await updateSystemDefaultLanguage(changedValues.language)

        // Update the store's global language
        await loadGlobalDefaultLanguage()

        message.success('Default language updated successfully')
      } catch {
        console.error('Failed to update default language')
        // Error is handled by the store
        form.setFieldsValue({ language: globalDefaultLanguage })
      }
    }
  }

  if (isDesktopApp) {
    return (
      <Card>
        <div className="text-center">
          <Title level={4}>Admin Appearance Settings</Title>
          <Text type="secondary">
            Admin appearance settings are disabled in desktop mode
          </Text>
        </div>
      </Card>
    )
  }

  return (
    <Space direction="vertical" size="large" style={{ width: '100%' }}>
      <Title level={3}>{t('admin.appearanceSettings')}</Title>

      <Card title={t('admin.defaultSystemSettings')}>
        <Form
          form={form}
          onValuesChange={handleFormChange}
          initialValues={{
            language: globalDefaultLanguage,
          }}
        >
          <Space direction="vertical" size="middle" style={{ width: '100%' }}>
            <Flex
              justify="space-between"
              align={isMobile ? 'flex-start' : 'center'}
              vertical={isMobile}
              gap={isMobile ? 'small' : 0}
            >
              <div>
                <Text strong>Default Language</Text>
                <div>
                  <Text type="secondary">
                    Set the default language for new users and the system
                    interface.
                  </Text>
                </div>
              </div>
              <Form.Item name="language" style={{ margin: 0 }}>
                <Select
                  loading={updating}
                  disabled={!canEditAppearance}
                  style={{ minWidth: 120 }}
                  options={[
                    { value: 'en', label: t('appearance.english') },
                    { value: 'vi', label: t('appearance.vietnamese') },
                  ]}
                />
              </Form.Item>
            </Flex>
          </Space>
        </Form>
      </Card>
    </Space>
  )
}
