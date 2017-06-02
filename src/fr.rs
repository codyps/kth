
pub fn select<T: Ord>(array: &mut [T], k: usize)
{
    let mut left = 0;
    let mut right = array.len();

    while right > left {
        if right - left > 600 {
            let n = (right - left + 1) as f32;
            let i = (k - left + 1) as f32;
            let z = n.ln();
            let s = 0.5 * (2*z/3).exp();
            let sd = 0.5 * (z*s*(n-s)/n).sqrt() * (i-n/2).sign();
            let nl = max(left, k - i * s/n + sd);
            let nr = min(right, k + (n - i) * s/n + sd);
            select(array[nl..nr], k);
        }

        let t = array[k];
        let mut i = left;
        let mut j = right;
        array.swap(left, k);
        if array[right] > t {
            array.swap(right, left);
        }

        while i < j {
            array.swap(i,j);
            i += 1;
            j -= 1;

            while array[i] < t {
                i += 1;
            }
            while array[j] > t {
                j -= 1;
            }
        }

        if array[left] == t {
            array.swap(left, j);
        } else {
            j += 1;
            array.swap(j, right);
        }

        if j <= k {
            left = j + 1;
        }
        if k <= j {
            right = j - 1;
        }
    }
}
