use crate::clamp;

#[test]
fn test_clamp_algo(){
	let result = clamp(3600,3100);
	println!("result for clamp {}",&result);
	assert_eq!(1,result,"Difficulty does not need to increase by 1");
}