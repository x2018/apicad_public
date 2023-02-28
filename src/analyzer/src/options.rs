use std::path::PathBuf;

pub trait GeneralOptions {
    fn use_serial(&self) -> bool;

    fn use_batch(&self) -> bool;
}

pub trait IOOptions {
    // Basic IO option
    fn input_path(&self) -> PathBuf;

    fn output_path(&self) -> PathBuf;

    fn basename_of_bc_file(&self) -> Option<&str>;

    fn with_name_of_bc_file(&self, path: PathBuf) -> PathBuf {
        match self.basename_of_bc_file() {
            Some(package) => path.join(package),
            None => path,
        }
    }

    // Options related with slice
    fn slice_dir(&self) -> PathBuf {
        self.output_path().join("slices")
    }

    fn slice_target_dir(&self, target: &str) -> PathBuf {
        self.with_name_of_bc_file(self.slice_dir().join(target))
    }

    fn slice_target_file_path(&self, target: &str, slice_id: usize) -> PathBuf {
        self.slice_target_dir(target).join(format!("{}.json", slice_id))
    }

    fn slice_target_package_dir(&self, target: &str, package: &str) -> PathBuf {
        self.slice_dir().join(target).join(package)
    }

    fn slice_target_package_file_path(&self, target: &str, package: &str, slice_id: usize) -> PathBuf {
        self.slice_target_package_dir(target, package)
            .join(format!("{}.json", slice_id))
    }

    // Options related with trace
    fn trace_dir(&self) -> PathBuf {
        self.output_path().join("traces")
    }

    fn trace_target_dir(&self, target: &str) -> PathBuf {
        self.with_name_of_bc_file(self.trace_dir().join(target))
    }

    fn trace_target_slice_dir(&self, target: &str, slice_id: usize) -> PathBuf {
        self.trace_target_dir(target).join(slice_id.to_string())
    }

    fn trace_target_slice_file_path(&self, target: &str, slice_id: usize, trace_id: usize) -> PathBuf {
        self.trace_target_slice_dir(target, slice_id)
            .join(format!("{}.json", trace_id))
    }

    fn trace_target_package_slice_dir(&self, target: &str, package: &str, slice_id: usize) -> PathBuf {
        self.trace_dir().join(target).join(package).join(slice_id.to_string())
    }

    fn trace_target_package_slice_file_path(
        &self,
        target: &str,
        package: &str,
        slice_id: usize,
        trace_id: usize,
    ) -> PathBuf {
        self.trace_target_package_slice_dir(target, package, slice_id)
            .join(format!("{}.json", trace_id))
    }

    // Options related with feature
    fn feature_dir(&self) -> PathBuf {
        self.output_path().join("features")
    }

    fn feature_target_dir(&self, target: &str) -> PathBuf {
        self.with_name_of_bc_file(self.feature_dir().join(target))
    }

    fn feature_target_slice_dir(&self, target: &str, slice_id: usize) -> PathBuf {
        self.feature_target_dir(target).join(slice_id.to_string())
    }

    fn feature_target_slice_file_path(&self, target: &str, slice_id: usize, trace_id: usize) -> PathBuf {
        self.feature_target_slice_dir(target, slice_id)
            .join(format!("{}.fea.json", trace_id))
    }

    fn feature_target_package_slice_dir(&self, target: &str, package: &str, slice_id: usize) -> PathBuf {
        self.feature_dir().join(target).join(package).join(slice_id.to_string())
    }

    fn feature_target_package_slice_file_path(
        &self,
        target: &str,
        package: &str,
        slice_id: usize,
        trace_id: usize,
    ) -> PathBuf {
        self.feature_target_package_slice_dir(target, package, slice_id)
            .join(format!("{}.fea.json", trace_id))
    }
}
