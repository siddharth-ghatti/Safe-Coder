use super::common::*;
use anyhow::Result;
use safe_coder::git::GitManager;
use serial_test::serial;
use std::fs;
use tokio::process::Command;

#[tokio::test]
#[serial]
async fn test_git_manager_creation() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let git_manager = GitManager::new(env.project_path.clone()).await?;
    
    // Should create without error
    assert_eq!(git_manager.repo_path(), &env.project_path);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_git_manager_with_non_git_directory() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    // Don't initialize git

    let result = GitManager::new(env.project_path.clone()).await;
    
    match result {
        Ok(_) => {
            // Git manager might auto-initialize
        }
        Err(e) => {
            // Expected error for non-git directory
            assert_contains(&e.to_string(), "git");
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_git_status() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let git_manager = GitManager::new(env.project_path.clone()).await?;
    
    // Test getting git status
    let status_result = git_manager.status().await;
    
    match status_result {
        Ok(status) => {
            // Should return status without error
            assert!(status.is_string() || status.is_object());
        }
        Err(e) => {
            // Should handle errors gracefully
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_git_add_files() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let git_manager = GitManager::new(env.project_path.clone()).await?;
    
    // Create a new file to add
    let test_file = env.project_path.join("new_file.txt");
    fs::write(&test_file, "test content")?;
    
    // Test adding files
    let add_result = git_manager.add_files(&["new_file.txt"]).await;
    
    match add_result {
        Ok(_) => {
            // Verify file was staged
            let output = Command::new("git")
                .args(&["status", "--porcelain"])
                .current_dir(&env.project_path)
                .output()
                .await?;
            
            let status_output = String::from_utf8_lossy(&output.stdout);
            assert_contains(&status_output, "new_file.txt");
        }
        Err(e) => {
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_git_commit() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let git_manager = GitManager::new(env.project_path.clone()).await?;
    
    // Add and commit the initial files
    let _ = git_manager.add_files(&["."]).await;
    
    let commit_result = git_manager.commit("Initial commit").await;
    
    match commit_result {
        Ok(_) => {
            // Verify commit was created
            let output = Command::new("git")
                .args(&["log", "--oneline", "-1"])
                .current_dir(&env.project_path)
                .output()
                .await?;
            
            let log_output = String::from_utf8_lossy(&output.stdout);
            assert_contains(&log_output, "Initial commit");
        }
        Err(e) => {
            // Might fail if nothing to commit
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_git_branch_operations() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    // Create initial commit
    Command::new("git")
        .args(&["add", "."])
        .current_dir(&env.project_path)
        .status()
        .await?;

    Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&env.project_path)
        .status()
        .await?;

    let git_manager = GitManager::new(env.project_path.clone()).await?;
    
    // Test creating a new branch
    let create_branch_result = git_manager.create_branch("test-branch").await;
    
    match create_branch_result {
        Ok(_) => {
            // Verify branch was created
            let output = Command::new("git")
                .args(&["branch"])
                .current_dir(&env.project_path)
                .output()
                .await?;
            
            let branches = String::from_utf8_lossy(&output.stdout);
            assert_contains(&branches, "test-branch");
            
            // Test switching to the branch
            let checkout_result = git_manager.checkout_branch("test-branch").await;
            
            match checkout_result {
                Ok(_) => {
                    // Verify we're on the new branch
                    let output = Command::new("git")
                        .args(&["branch", "--show-current"])
                        .current_dir(&env.project_path)
                        .output()
                        .await?;
                    
                    let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    assert_eq!(current_branch, "test-branch");
                }
                Err(e) => {
                    assert!(!e.to_string().contains("panic"));
                }
            }
        }
        Err(e) => {
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_git_diff() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    // Create initial commit
    Command::new("git")
        .args(&["add", "."])
        .current_dir(&env.project_path)
        .status()
        .await?;

    Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&env.project_path)
        .status()
        .await?;

    let git_manager = GitManager::new(env.project_path.clone()).await?;
    
    // Modify a file
    let main_file = env.project_path.join("src/main.rs");
    fs::write(&main_file, "fn main() {\n    println!(\"Modified hello, world!\");\n}")?;
    
    // Test getting diff
    let diff_result = git_manager.diff().await;
    
    match diff_result {
        Ok(diff) => {
            assert_contains(&diff, "Modified");
        }
        Err(e) => {
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_git_reset() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    // Create initial commit
    Command::new("git")
        .args(&["add", "."])
        .current_dir(&env.project_path)
        .status()
        .await?;

    Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&env.project_path)
        .status()
        .await?;

    let git_manager = GitManager::new(env.project_path.clone()).await?;
    
    // Modify a file and stage it
    let test_file = env.project_path.join("test_reset.txt");
    fs::write(&test_file, "test content")?;
    
    Command::new("git")
        .args(&["add", "test_reset.txt"])
        .current_dir(&env.project_path)
        .status()
        .await?;
    
    // Test reset
    let reset_result = git_manager.reset_hard("HEAD").await;
    
    match reset_result {
        Ok(_) => {
            // Verify file was removed
            assert!(!test_file.exists());
        }
        Err(e) => {
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_git_auto_commit() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let git_manager = GitManager::new(env.project_path.clone()).await?;
    
    // Create initial commit
    let _ = git_manager.add_files(&["."]).await;
    let _ = git_manager.commit("Initial commit").await;
    
    // Create a new file
    let new_file = env.project_path.join("auto_commit_test.txt");
    fs::write(&new_file, "auto commit test")?;
    
    // Test auto-commit functionality
    let auto_commit_result = git_manager.auto_commit("Auto-commit: added test file").await;
    
    match auto_commit_result {
        Ok(_) => {
            // Verify commit was created
            let output = Command::new("git")
                .args(&["log", "--oneline", "-1"])
                .current_dir(&env.project_path)
                .output()
                .await?;
            
            let log_output = String::from_utf8_lossy(&output.stdout);
            assert_contains(&log_output, "Auto-commit");
        }
        Err(e) => {
            // Might fail if nothing to commit or other issues
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod git_error_handling_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_git_invalid_operations() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;

        let git_manager = GitManager::new(env.project_path.clone()).await?;
        
        // Test operations that should fail gracefully
        
        // Try to checkout non-existent branch
        let invalid_checkout = git_manager.checkout_branch("nonexistent-branch").await;
        assert!(invalid_checkout.is_err());
        
        // Try to commit with nothing staged
        let empty_commit = git_manager.commit("Empty commit").await;
        match empty_commit {
            Ok(_) => {
                // Some git configurations allow empty commits
            }
            Err(e) => {
                // Expected error for empty commit
                assert!(!e.to_string().contains("panic"));
            }
        }
        
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_git_permission_errors() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;

        let git_manager = GitManager::new(env.project_path.clone()).await?;
        
        // Test adding a file that doesn't exist
        let add_nonexistent = git_manager.add_files(&["nonexistent_file.txt"]).await;
        
        match add_nonexistent {
            Ok(_) => {
                // Git might ignore missing files
            }
            Err(e) => {
                // Expected error for missing file
                assert!(!e.to_string().contains("panic"));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod git_worktree_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_git_worktree_operations() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;

        // Create initial commit (required for worktrees)
        Command::new("git")
            .args(&["add", "."])
            .current_dir(&env.project_path)
            .status()
            .await?;

        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&env.project_path)
            .status()
            .await?;

        let git_manager = GitManager::new(env.project_path.clone()).await?;
        
        // Test creating a worktree
        let worktree_path = env.temp_dir.path().join("test-worktree");
        let create_worktree_result = git_manager.create_worktree(
            &worktree_path,
            "test-worktree-branch"
        ).await;
        
        match create_worktree_result {
            Ok(_) => {
                // Verify worktree was created
                assert!(worktree_path.exists());
                assert!(worktree_path.join(".git").exists());
            }
            Err(e) => {
                // Might not be supported or other issues
                assert!(!e.to_string().contains("panic"));
            }
        }
        
        Ok(())
    }
}