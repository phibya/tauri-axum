import { UploadOutlined } from '@ant-design/icons'
import { App, Button, Card, Flex, Progress, Typography, Upload } from 'antd'
import React, { useEffect } from 'react'
import { useParams } from 'react-router-dom'
import { loadProjectFiles, Stores, uploadFilesToProject } from '../../../store'
import { FileCard } from './FileCard'

const { Text } = Typography

export const ProjectKnowledgeCard: React.FC = () => {
  const { message } = App.useApp()
  const { projectId } = useParams<{ projectId: string }>()

  // Projects store
  const { currentProject } = Stores.Projects

  // Project files store
  const { uploading, uploadProgress, showProgress } = Stores.ProjectFiles

  // Get files for this project
  const projectFiles = projectId
    ? Stores.ProjectFiles.filesByProject[projectId] || []
    : []

  useEffect(() => {
    if (projectId) {
      // Load project files
      loadProjectFiles(projectId).catch((error: any) => {
        console.error('Failed to load project files:', error)
      })
    }
  }, [projectId])

  const handleFileUpload = async (files: globalThis.File[]) => {
    if (!currentProject || !projectId) return

    try {
      await uploadFilesToProject(projectId, files)
      message.success(`${files.length} file(s) uploaded successfully`)
    } catch (error) {
      console.error('Failed to upload files:', error)
      message.error('Failed to upload files')
    }
  }

  return (
    <Card
      title="Project knowledge"
      className="w-96 !my-1 overflow-y-hidden flex flex-col"
      classNames={{
        body: 'flex flex-col relative overflow-y-hidden flex-1',
      }}
    >
      <Upload.Dragger
        multiple
        beforeUpload={(_, fileList) => {
          handleFileUpload(fileList).catch(error => {
            console.error('Failed to upload files:', error)
          })
          return false
        }}
        showUploadList={false}
        className="!p-0 !m-0
        [&_.ant-upload-drag]:!border-none [&_.ant-upload-drag]:!bg-transparent
        [&_.ant-upload-drag-hover]:!border-dashed
        absolute left-2 right-2 top-3 bottom-2
        "
        openFileDialogOnClick={false}
      />
      {/* Project Instructions */}
      <Flex className="gap-1 flex-col">
        <Typography.Title level={5}>Project Instructions</Typography.Title>
        <Card>
          <Text type="secondary">
            {currentProject?.instruction ||
              currentProject?.description ||
              'No instructions provided for this project.'}
            <Button type="link" size="small" style={{ pointerEvents: 'auto' }}>
              Edit
            </Button>
          </Text>
        </Card>
      </Flex>

      {/* Upload Progress */}
      {showProgress && (
        <div className={'py-0 flex flex-col gap-1 pt-2'}>
          <Text strong>Uploading files...</Text>
          <div className={'py-4 flex flex-col gap-2'}>
            {uploadProgress.map((progress, index) => (
              <div key={index}>
                <Text style={{ fontSize: '12px' }}>{progress.filename}</Text>
                <Progress
                  percent={progress.progress}
                  size="small"
                  status={progress.status === 'error' ? 'exception' : 'active'}
                />
                {progress.error && (
                  <Text type="danger" style={{ fontSize: '10px' }}>
                    {progress.error}
                  </Text>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Documents */}
      <Flex justify="space-between" align="center" className={'!mt-3'}>
        <Typography.Title level={5}>Documents</Typography.Title>
        <Button
          icon={<UploadOutlined />}
          loading={uploading}
          style={{ pointerEvents: 'auto' }}
        >
          + Add Files
        </Button>
      </Flex>

      <div className={'overflow-y-auto mt-3 flex-1'}>
        <div className="flex gap-2 flex-wrap">
          {projectFiles.map(file => (
            <FileCard key={file.id} file={file} />
          ))}
        </div>
      </div>
    </Card>
  )
}
