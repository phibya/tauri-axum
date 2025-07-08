import {
  Card,
  Divider,
  Flex,
  Form,
  message,
  Select,
  Space,
  Typography,
} from 'antd'
import { useEffect, useState } from 'react'
import { useAppearanceSettings } from '../../../../store'
import { isDesktopApp } from '../../../../api/core'
import { Permission, usePermissions } from '../../../../permissions'

const { Title, Text } = Typography

export function AdminAppearanceSettings() {
  const [form] = Form.useForm()
  const [isMobile, setIsMobile] = useState(false)
  const { hasPermission } = usePermissions()
  const {
    language,
    setLanguage,
    loading,
  } = useAppearanceSettings()

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
      language,
    })
  }, [language, form])

  const handleFormChange = async (changedValues: any) => {
    if ('language' in changedValues) {
      if (!canEditAppearance) {
        message.error('You do not have permission to change system settings')
        form.setFieldsValue({ language })
        return
      }
      
      try {
        await setLanguage(changedValues.language)
        message.success('Default language updated successfully')
      } catch {
        message.error('Failed to update default language')
        form.setFieldsValue({ language })
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
      <Title level={3}>Admin Appearance Settings</Title>

      <Card title="Default System Settings">
        <Form
          form={form}
          onValuesChange={handleFormChange}
          initialValues={{
            language,
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
                    Set the default language for new users and the system interface.
                  </Text>
                </div>
              </div>
              <Form.Item name="language" style={{ margin: 0 }}>
                <Select
                  loading={loading}
                  disabled={!canEditAppearance}
                  style={{ minWidth: 120 }}
                  options={[
                    { value: 'en', label: 'English' },
                    { value: 'vi', label: 'Tiếng Việt' },
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